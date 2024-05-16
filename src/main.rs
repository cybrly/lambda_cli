use clap::{Parser, Subcommand};
use colored::Colorize;
use prettytable::{row, Table};
use reqwest::blocking::Client;
use reqwest::header::AUTHORIZATION;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::thread;
use std::time::{Duration, Instant};
use chrono::Local;
use dotenv::dotenv;
use crossterm::{execute, terminal::{Clear, ClearType}, cursor::MoveTo};
use std::io::{stdout, Write};

/// Simple program to interact with Lambda Labs GPU cloud
#[derive(Parser)]
#[command(name = "lambda")]
#[command(about = "A command-line tool for Lambda Labs cloud GPU API", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// List all available GPU instances
    List,
    /// Start a GPU instance with the specified SSH key
    Start {
        #[arg(short, long)]
        gpu: String,
        #[arg(short, long)]
        ssh: String,
    },
    /// Stop a specified GPU instance
    Stop {
        #[arg(short, long)]
        gpu: String,
    },
    /// List all running GPU instances
    Running,
    /// Continuously find and start a GPU instance when it becomes available
    Find {
        #[arg(short, long)]
        gpu: String,
        #[arg(short, long, default_value = "")]
        ssh: String,
        #[arg(short, long, default_value_t = 10)]
        sec: u64,
    },
}

#[derive(Deserialize, Debug)]
struct ApiResponse<T> {
    data: T,
}

#[derive(Deserialize, Debug)]
struct Instance {
    id: Option<String>,
    status: Option<String>,
    ip: Option<String>,
    ssh_key_names: Option<Vec<String>>,
}

#[derive(Deserialize, Debug)]
struct LaunchResponse {
    instance_ids: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct InstanceTypeResponse {
    instance_type: InstanceType,
    regions_with_capacity_available: Vec<Region>,
}

#[derive(Deserialize, Debug, Clone)]
struct InstanceType {
    description: String,
    price_cents_per_hour: i32,
    specs: InstanceSpecs,
}

#[derive(Deserialize, Debug, Clone)]
struct InstanceSpecs {
    vcpus: u32,
    memory_gib: u32,
    storage_gib: u32,
}

#[derive(Deserialize, Debug, Clone)]
struct Region {
    name: String,
    description: String,
}

fn main() {
    dotenv().ok();
    let api_key = env::var("LAMBDA_API_KEY").expect("LAMBDA_API_KEY must be set");
    let client = Client::new();

    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::List) => {
            list_instances(&client, &api_key);
        }
        Some(Commands::Start { gpu, ssh }) => {
            start_instance(&client, &api_key, gpu, ssh);
        }
        Some(Commands::Stop { gpu }) => {
            stop_instance(&client, &api_key, gpu);
        }
        Some(Commands::Running) => {
            list_running_instances(&client, &api_key);
        }
        Some(Commands::Find { gpu, ssh, sec }) => {
            find_and_start_instance(&client, &api_key, gpu, ssh, *sec);
        }
        None => {
            validate_api_key(&client, &api_key);
        }
    }
}

fn validate_api_key(client: &Client, api_key: &str) {
    let url = "https://cloud.lambdalabs.com/api/v1/instances";
    let response = client.get(url)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .send()
        .expect("Failed to validate API key");

    if response.status().is_success() {
        println!("API key is valid");
    } else {
        println!("Failed to validate API key: {}", response.status());
    }
}

fn list_instances(client: &Client, api_key: &str) {
    let url = "https://cloud.lambdalabs.com/api/v1/instance-types";
    let response: ApiResponse<HashMap<String, InstanceTypeResponse>> = client.get(url)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .send()
        .expect("Failed to list instances")
        .json()
        .expect("Failed to parse response");

    let mut table = Table::new();
    table.add_row(row!["Instance Type", "Description", "Price (cents/hour)", "vCPUs", "Memory (GiB)", "Storage (GiB)", "Available Regions"]);

    for (key, instance_type_response) in response.data {
        if !instance_type_response.regions_with_capacity_available.is_empty() {
            let regions: Vec<String> = instance_type_response.regions_with_capacity_available
                .iter()
                .map(|region| format!("{} ({})", region.name, region.description))
                .collect();

            table.add_row(row![
                key.green(),
                instance_type_response.instance_type.description.clone(),
                instance_type_response.instance_type.price_cents_per_hour.to_string().yellow(),
                instance_type_response.instance_type.specs.vcpus.to_string(),
                instance_type_response.instance_type.specs.memory_gib.to_string(),
                instance_type_response.instance_type.specs.storage_gib.to_string(),
                regions.join(", ").blue()
            ]);
        }
    }

    table.printstd();
}

fn start_instance(client: &Client, api_key: &str, gpu: &str, ssh: &str) {
    if let Some(instance_type_response) = get_instance_type_response(client, api_key, gpu) {
        let region_name = &instance_type_response.regions_with_capacity_available[0].name;

        let url = "https://cloud.lambdalabs.com/api/v1/instance-operations/launch";
        let payload = serde_json::json!({
            "region_name": region_name,
            "instance_type_name": gpu,
            "ssh_key_names": [ssh],
            "quantity": 1
        });

        let response_result = client.post(url)
            .header(AUTHORIZATION, format!("Bearer {}", api_key))
            .json(&payload)
            .send();

        match response_result {
            Ok(response) => {
                let response_text = response.text().unwrap_or_else(|_| "Failed to read response text".to_string());
                match serde_json::from_str::<ApiResponse<LaunchResponse>>(&response_text) {
                    Ok(parsed_response) => {
                        let instance_id = &parsed_response.data.instance_ids[0];
                        println!("Instance {} started in region {}. Waiting for it to become active...", instance_id, region_name);

                        std::thread::sleep(std::time::Duration::from_secs(120));

                        let instance = get_instance_details(client, api_key, instance_id);
                        match instance.ip {
                            Some(ip) => println!("Instance is active. SSH IP: {}", ip),
                            None => println!("Instance is active, but IP address is not available yet."),
                        }
                    }
                    Err(e) => {
                        println!("Failed to parse response: {}\nResponse text: {}", e, response_text);
                    }
                }
            }
            Err(e) => {
                println!("Request failed: {}", e);
            }
        }
    } else {
        println!("Instance type {} not found.", gpu);
    }
}

fn get_instance_type_response(client: &Client, api_key: &str, gpu: &str) -> Option<InstanceTypeResponse> {
    let url = "https://cloud.lambdalabs.com/api/v1/instance-types";
    let response: ApiResponse<HashMap<String, InstanceTypeResponse>> = client.get(url)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .send()
        .expect("Failed to get instance types")
        .json()
        .expect("Failed to parse response");

    response.data.get(gpu).cloned()
}

fn stop_instance(client: &Client, api_key: &str, gpu: &str) {
    let url = "https://cloud.lambdalabs.com/api/v1/instance-operations/terminate";
    let payload = serde_json::json!({
        "instance_ids": [gpu]
    });

    client.post(url)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .expect("Failed to stop instance");

    println!("Instance {} stopped", gpu);
}

fn list_running_instances(client: &Client, api_key: &str) {
    let url = "https://cloud.lambdalabs.com/api/v1/instances";
    let response: ApiResponse<Vec<Instance>> = client.get(url)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .send()
        .expect("Failed to list running instances")
        .json()
        .expect("Failed to parse response");

    let mut table = Table::new();
    table.add_row(row!["Instance ID", "Status", "IP Address", "SSH Key Names"]);

    for instance in response.data {
        table.add_row(row![
            instance.id.unwrap_or_else(|| "N/A".to_string()).green(),
            instance.status.unwrap_or_else(|| "N/A".to_string()).yellow(),
            instance.ip.unwrap_or_else(|| "N/A".to_string()).blue(),
            instance.ssh_key_names.unwrap_or_else(|| vec!["N/A".to_string()]).join(", ").purple()
        ]);
    }

    table.printstd();
}

fn find_and_start_instance(client: &Client, api_key: &str, gpu: &str, ssh: &str, sec: u64) {
    println!("Looking for available instances of type {}...", gpu);

    loop {
        let start_time = Instant::now();
        let check_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let mut table = Table::new();
        table.add_row(row!["Last Checked", "Status", "Next Check In (s)"]);

        if let Some(instance_type_response) = get_instance_type_response(client, api_key, gpu) {
            if !instance_type_response.regions_with_capacity_available.is_empty() {
                let regions: Vec<String> = instance_type_response.regions_with_capacity_available
                    .iter()
                    .map(|region| format!("{} ({})", region.name, region.description))
                    .collect();

                println!("Found available {} in region(s): {:?}", gpu, regions);
                start_instance(client, api_key, gpu, ssh);
                break;
            }
        }
        
        let next_check_in = sec.saturating_sub(start_time.elapsed().as_secs());
        table.add_row(row![
            check_time,
            "No available instances found".red(),
            next_check_in.to_string().yellow()
        ]);

        // Clear the screen and print the updated table
        execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0)).unwrap();
        table.printstd();

        thread::sleep(Duration::from_secs(next_check_in));
    }
}

fn get_instance_details(client: &Client, api_key: &str, instance_id: &str) -> Instance {
    let url = format!("https://cloud.lambdalabs.com/api/v1/instances/{}", instance_id);
    let response_result = client.get(&url)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .send();

    match response_result {
        Ok(response) => {
            let response_text = response.text().unwrap_or_else(|_| "Failed to read response text".to_string());
            match serde_json::from_str::<ApiResponse<Instance>>(&response_text) {
                Ok(parsed_response) => parsed_response.data,
                Err(e) => {
                    println!("Failed to parse response: {}\nResponse text: {}", e, response_text);
                    panic!("Failed to get instance details");
                }
            }
        }
        Err(e) => {
            println!("Request failed: {}", e);
            panic!("Failed to get instance details");
        }
    }
}
