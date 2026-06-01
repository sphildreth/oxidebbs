# Setup Wizard

OxideBBS includes an interactive setup command for creating a new local board
configuration.

Run it from the repository root:

```bash
cargo run -p oxidebbs-server -- setup
```

By default the wizard writes:

```text
config/oxidebbs.toml
```

Use `--output` to choose another config path:

```bash
cargo run -p oxidebbs-server -- setup --output config/my-bbs.toml
```

The wizard asks for:

- board name
- tagline
- sysop name
- timezone
- telnet bind address
- node count
- database path
- whether to include the placeholder example door definition

Press Enter at a prompt to accept the value shown in brackets.

## Created Paths

Setup creates the starter directories used by the generated config:

```text
config/
data/
assets/ansi/
assets/screens/
doors/
runtime/
logs/
```

The command will not overwrite an existing output file unless `--force` is
provided:

```bash
cargo run -p oxidebbs-server -- setup --force
```

## After Setup

Validate the generated config:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml check
```

Then inspect local admin commands:

```bash
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml admin users
cargo run -p oxidebbs-server -- --config config/oxidebbs.toml admin nodes
```

The setup command may include a sample door definition, but it does not install
or bundle any DOS door binaries.
