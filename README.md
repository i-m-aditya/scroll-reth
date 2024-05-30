# Scroll L2 Follower Node

This project implements a follower node for Scroll L2 that participates in the public mempool of transactions but does not participate in the consensus of block building. It leverages the highly modular `reth` stack and `alloy` to achieve its functionality. 

## Overview

The Scroll L2 follower node is designed to:

- Use the modular components of the `reth` stack and `alloy` for streamlined and efficient development.
- Serve as an experimental setup for further enhancements and optimizations.

## Installation

### Prerequisites

- Rust (latest stable version)
- Cargo (latest version)
- Git

### Steps

1. Clone the repository:

    ```bash
    git clone https://github.com/i-m-aditya/scroll-reth.git
    cd scroll-reth
    ```

2. Build the project:

    ```bash
    cargo build --release
    ```

3. Run the follower node:

    ```bash
    ./target/release/scroll-reth
    ```
    
## Experimental Status

**Note:** This project is currently experimental and a work in progress. There are several aspects that are still under development, and the codebase is subject to significant changes. Use this project with caution and expect potential issues and bugs. Contributions and feedback are welcome to help improve and stabilize the project.
