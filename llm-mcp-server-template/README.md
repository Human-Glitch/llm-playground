# LLM MCP Server Template

A template project for creating Message Control Protocol (MCP) servers using FastMCP, designed for Large Language Model integrations.

<img width="1445" alt="image" src="https://github.com/user-attachments/assets/fa185edf-bef0-45f2-a4a6-3ba16eea0e8b" />


## Features

- Built with FastMCP for efficient MCP server implementation
- Python 3.13+ support
- Async-first architecture
- Integrated CLI tools via MCP

## Prerequisites

- Python 3.13 or higher
- Poetry for dependency management

## Installation

1. Clone the repository
2. Install dependencies:
```bash
poetry install
```

## Usage

The project uses Poetry for dependency management and running commands.

### Development

To start working with the project:

```bash
poetry run fastmcp dev server.py
```

Navigate available tools calls with the MCP Inspector Tool at http://127.0.0.1:6274

## Dependencies

- fastmcp: MCP server implementation
- mcp: Message Control Protocol utilities
- httpx: HTTP client library
- uv: Fast Python package installer and resolver

## License

[MIT License](LICENSE)

## Authors

- Kody Buss <kody.buss@gmail.com>
