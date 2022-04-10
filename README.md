# aliesce

Write, save and run scripts in multiple languages from a single source file.

## Why?

For smoother development of related code, to keep the source about as closely collocated as possible, or for practice, for a more direct absorption of syntax and idiom.

## How?

Create a file for your scripts. Give it any name, and any extension or none. Use the current default name - 'src.txt' - to avoid passing an argument later.

Write your scripts.

Precede each script in the file with a tag line starting by default `###`. Include in the tag line the following elements:

- the file extension for that language, or the full output filename including extension, or the full output path including directory and extension
- the command to run the file, if any
- any arguments to pass to that command

Ensure each element is separated by one or more spaces.

For example, a possible tag line for a script in Elixir:

```
### exs elixir -r setup
```

This tells aliesce to save the script following the tag line in a file with the `exs` extension, then run that with the `elixir` command, applying one option, to require a file named 'setup'.

### Output paths

The basename of the output file will be the basename of the source file, i.e. 'src' by default. The file is saved by default to a folder in the current directory named `scripts`, which is created if not present.

For an output file named 'script.exs', the following would be used:

```
### script.exs elixir -r setup
```

For an output directory named 'output' holding 'script.exs':

```
### output/script.exs elixir -r setup
```

### Running aliesce

If aliesce is compiled and ready to go (see [Getting started](#getting-started) below), run the `aliesce` command, adding the source file name if not the default.

For example, for a source file named only 'src':

```shell
aliesce src
```

The script files are saved and run in order of appearance in the source file.

### Avoiding stages

To avoid a script being saved and run, simply include the `!` character as a tag line element, before the extension or full output filename or path:

```
### ! script.exs elixir -r setup
```

To save the script but avoid the run stage, include the `!` character as an element after the extension or full output filename or path but before the command to run the code:

```
### script.exs ! elixir -r setup
```

## Getting started

The `dir`, `src` and `tag` defaults are defined close to the top of the source file, i.e. 'src/main.rs', should you prefer to modify any pre-compilation.

With Rust and Cargo installed, at the root of the aliesce directory run `cargo build --release` to compile. The binary is created in the 'target/release' directory.

The binary can be run with the command `./aliesce` while in the same directory, and from elsewhere using the pattern `path/to/aliesce`. It can be run from any directory with `aliesce` by placing it in the '/bin' or '/usr/bin' directory.

## Making changes

Running the tests after making changes and adding tests to cover new behaviour is recommended.

### Tests

The tests can be run with the following command:

```shell
cargo test
```

The tests themselves are in the test module at the base of the file.

## Development plan

The following are the expected next steps in the development of the code base. The general medium-term aim is a convenient parallel scripting tool. Pull requests are welcome for these and other potential improvements.

- allow for an alternative output directory with the default basename
- allow for arguments to scripts run from aliesce
- provide tag line options for:
  - multiple save paths
  - auxiliary commands
- provide CLI options for:
  - output verbosity
  - importing scripts
- refactor as more idiomatic
- improve error handling
- add fuller testing
