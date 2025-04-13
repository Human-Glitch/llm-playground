# GitHub Release Notes Formatter

A Rust CLI tool that automatically formats GitHub release notes using OpenAI's GPT models.

## Features

- Automatically creates and updates GitHub releases
- Formats release notes using OpenAI's GPT models
- Supports custom formatting templates
- Handles tag management and release updates

## Prerequisites

- Rust (1.56 or later)
- GitHub Personal Access Token with repo permissions
- OpenAI API Key

## Installation

1. Clone the repository:
```bash
git clone https://github.com/Human-Glitch/llm-playground.git
cd github-releaser-llm
```

2. Build the project:
```bash
cargo build --release
```

## Configuration

Create a `.env` file in the project root with the following variables:

```env
GITHUB_TOKEN=your_github_token
OPENAI_API_KEY=your_openai_api_key
```

## Usage

```bash
github-releaser-llm --tag v1.2.3
```

This will:
1. Delete existing release and tag if they exist
2. Create a new tag from the specified release branch
3. Create a new release with auto-generated notes
4. Format the notes using OpenAI
5. Update the release with formatted notes

## Release Notes Format

The tool formats release notes following this template:
```
[PDE-3441](https://company.atlassian.net/browse/PDE-3441) Fixed an issue by @Human-Glitch in #2329
```

Notes are grouped by ticket type (PD, PDE, PRDY) and sorted by ticket number.

## License

MIT License - see the [LICENSE](LICENSE) file for details

## Credits

Created by @Human-Glitch
