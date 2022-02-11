# aliesce

Write, save and run scripts in multiple languages from a single source file.

## Why?

For smoother development of related code, to keep the source about as closely collocated as possible, or for practice, for a more direct absorption of syntax and idiom.

## How?

Create a file with any name, and any extension or none. Use the current default - 'src.txt' - to avoid passing an argument later.

Precede each script in the file with a tag line starting by default `###`. Include in the tag line the following elements:

- the file extension for that language
- the command to run the file, if any
- any arguments to pass to that command

Ensure each element is separated by one or more spaces.

For example, for Elixir:

```
### exs elixir -r setup
```

This tells aliesce to save the subsequent script in a file with the `exs` extension then run that with the `elixir` command, applying one option, to require a file named 'setup'.

If aliesce is compiled and ready to go (see [Getting started](#getting-started) below), run the `aliesce` command, adding the source file name if not the default.

For example, for a source file named only 'src':

```shell
aliesce src
```

The scripts are saved by default to a folder in the current directory named `scripts`, which is created if not present. The script files are run in order of appearance in the source file.

### Avoiding stages

To avoid a script being saved and run, simply include the `!` character as a tag line element, before the extension:

```
### ! exs elixir -r setup
```

To save the script but avoid the run stage, include the `!` character as an element after the extension but before the command to run the code:

```
### exs ! elixir -r setup
```

## Getting started

The `dir`, `src` and `tag` defaults are defined close to the top of the source file, i.e. 'src/main.rs', should you prefer to modify any pre-compilation.

With Rust and Cargo installed, at the root of the aliesce directory run `cargo build --release` to compile. The binary is created in the 'target/release' directory.

The binary can be run with the command `./aliesce` while in the same directory, and from elsewhere using the pattern `path/to/aliesce`. It can be run from any directory with `aliesce` by placing it in the '/bin' or '/usr/bin' directory.
