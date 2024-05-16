# Lambda CLI

Lambda CLI is a command-line tool for interacting with the Lambda Labs cloud GPU API.

## Features

- Validate the API key
- List all available GPU instances
- Start a GPU instance with the specified SSH key
- Stop a specified GPU instance
- List all running GPU instances
- Continuously find and start a GPU instance when it becomes available

## Installation

To use Lambda CLI, you need to have Rust and Cargo installed. You can install Rust and Cargo by following the instructions [here](https://www.rust-lang.org/tools/install).

Clone the repository and navigate to the project directory:

```
git clone https://github.com/cybrly/lambda_cli.git

cd lambda_cli

cargo build --release
```


Usage

Before using Lambda CLI, set your Lambda API key as an environment variable:

```
export LAMBDA_API_KEY=your_api_key
```

Run the CLI tool:

```
./target/release/lambda_cli [COMMAND]
```

Commands

- list: List all available GPU instances.
- start --gpu <GPU_TYPE> --ssh <SSH_KEY>: Start a GPU instance with the specified SSH key.
- stop --gpu <GPU_INSTANCE_ID>: Stop a specified GPU instance.
- running: List all running GPU instances.
- find --gpu <GPU_TYPE> [--ssh <SSH_KEY>] [--sec <SECONDS>]: Continuously find and start a GPU instance when it becomes available.

Examples

Validate the API key:

```
./target/release/lambda_cli
```

List all available GPU instances:

```
./target/release/lambda_cli list
```

Start a GPU instance with the specified SSH key:

```
./target/release/lambda_cli start --gpu "gpu_1x_a10" --ssh "Chris"
```

Stop a specified GPU instance:

```
./target/release/lambda_cli stop --gpu "instance_id"
```

List all running GPU instances:

```
./target/release/lambda_cli running
```

Continuously find and start a GPU instance when it becomes available:

```
./target/release/lambda_cli find --gpu "8x_h100" --ssh "SSH_KEY_NAME" --sec 30
```
