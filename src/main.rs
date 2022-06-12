use std::env;
use std::fs;
use std::process;
use std::collections::HashMap;

/* define data structures */

#[derive(PartialEq, Eq)]
enum CLIOptVal {
  Bool(bool),
  Int(usize)
}

struct CLIOpt {
  word: String,
  char: String,
  vals: Vec<String>,
  desc: String,
  call: Box<dyn Fn(&[CLIOpt; 3], HashMap<String, CLIOptVal>, Vec<String>) -> HashMap<String, CLIOptVal>>
}

#[derive(Debug, PartialEq)]
struct OutputPath {
  dir: String,
  basename: String,
  ext: String
}

#[derive(Debug, PartialEq)]
struct Output {
  code: String,
  path: OutputPath,
  prog: String,
  args: Vec<String>,
  i: usize
}

/* define utility functions */

fn get_cli_option(word: &str, char: &str, vals: &[&str], desc: &str, call: &'static dyn Fn(&[CLIOpt; 3], HashMap<String, CLIOptVal>, Vec<String>) -> HashMap<String, CLIOptVal>) -> CLIOpt {
  CLIOpt {
    word: String::from(word),
    char: String::from(char),
    vals: if vals.len() > 0 { vals.iter().map(|&val|String::from(val)).collect::<Vec<String>>() } else { Vec::new() },
    desc: String::from(desc),
    call: Box::new(call)
  }
}

/* define CLI option applicators */

fn apply_cli_option_list(_0: &[CLIOpt; 3], mut opts_values: HashMap<String, CLIOptVal>, _1: Vec<String>) -> HashMap<String, CLIOptVal> {
  opts_values.insert( "is_list".to_string(), CLIOptVal::Bool(false) );
  opts_values
}

fn apply_cli_option_only(_: &[CLIOpt; 3], mut opts_values: HashMap<String, CLIOptVal>, vals: Vec<String>) -> HashMap<String, CLIOptVal> {
  let val = vals[0].trim().parse::<usize>().expect("parse script number for option 'only'");
  opts_values.insert("script_no".to_string(), CLIOptVal::Int(val));
  opts_values
}

fn apply_cli_option_help(cli_options: &[CLIOpt; 3], _0: HashMap<String, CLIOptVal>, _1: Vec<String>) -> HashMap<String, CLIOptVal> {

  let usage = "Usage: aliesce [--help/-h / [--list/-l] [--only/-o] [src]]";

  /* set value substrings and max length */
  let val_strs = cli_options.iter()
    .map(|cli_option|format!("{}", cli_option.vals.join(" ")))
    .collect::<Vec<String>>();
  let val_strs_max = val_strs.iter()
    .fold(0, |acc, val_str| if val_str.len() > acc { val_str.len() } else { acc });

  /* generate list of flags */
  let flags_list = cli_options.iter()
    .enumerate() /* yield also index (i) */
    .map(|(i, cli_option)|format!(" -{}, --{}  {:w$}  {}", cli_option.char, cli_option.word, val_strs[i], cli_option.desc, w = val_strs_max))
    .collect::<Vec<String>>()
    .join("\n");

  println!("{}\n{}\n{}", usage, String::from("Flags:"), flags_list);
  process::exit(0);
}

fn main() {

  /* initialize */

  /* set CLI options */
  let cli_options = [
    get_cli_option("list", "l", &[], "print for each script in the source file its number and tag line label and data, skipping the save and run stages", &apply_cli_option_list),
    get_cli_option("only", "o", &["NUMBER"], "include only script no. NUMBER", &apply_cli_option_only),
    get_cli_option("help", "h", &[], "show usage and a list of available flags then exit", &apply_cli_option_help)
  ];

  /* configure */

  /* get arguments and set count */
  let args: Vec<String> = env::args().collect();
  let mut args_count: usize = args.len().try_into().unwrap();

  /* provide for any flag passed */
  let mut opts_values = HashMap::new();
  let mut opts_count = 0;
  if args_count > 1 {
    for i in 0..cli_options.len() {
      for j in 1..args_count {
        if "--".to_owned() + &cli_options[i].word == args[j] || "-".to_owned() + &cli_options[i].char == args[j] {
          let vals_len = cli_options[i].vals.len();
          let vals = args[(j + 1)..(j + vals_len + 1)].to_vec();
          opts_values = (cli_options[i].call)(&cli_options, opts_values, vals);
          opts_count = opts_count + 1 + vals_len;
        };
      };
    };
  };
  args_count = args_count - opts_count;

  /* set source filename (incl. output basename), script tag and output directory */
  let src = if args_count > 1 { &args.last().unwrap() } else { "src.txt" };
  let tag = ("###", "#");
  let dir = "scripts";

  /* implement */

  /* get each script incl. tag line part, handle tag line, save and run */
  fs::read_to_string(src).expect(&format!("read source file '{}'", src))
    .split(tag.0)
    .skip(1) /* omit content preceding initial tag */
    .enumerate() /* yield also index (i) */
    .filter(|(i, _)| !opts_values.contains_key("script_no") || opts_values.get("script_no").unwrap() == &CLIOptVal::Int(i + 1) )
    .map(|(i, script_plus_tag_line_part)| parse(script_plus_tag_line_part, tag.1, dir, src, i, &opts_values))
    .for_each(apply)
}

fn parse<'a>(script_plus_tag_line_part: &'a str, tag_1: &str, dir: &str, src: &str, i: usize, opts_values: &HashMap<String, CLIOptVal>) -> Option<Output> {

  let mut lines = script_plus_tag_line_part.lines();
  let tag_line_part = lines.nth(0).unwrap().trim();

  /* get label and data from tag line */
  let tag_line_subparts = match tag_line_part.find(tag_1) { Some(i) => tag_line_part.split_at(i + 1), None => ("", tag_line_part) };
  let tag_line_label = tag_line_subparts.0.split(tag_1).nth(0).unwrap().trim();
  let tag_line_data = tag_line_subparts.1.trim();

  /* handle option selected - list */
  if opts_values.contains_key("is_list") {
    let tag_line_part_new = if tag_line_label.len() > 0 { format!("{}: {}", tag_line_label, tag_line_data) } else { tag_line_data.to_string() };
    println!("{}: {}", i + 1, tag_line_part_new);
    return None;
  };

  /* get items from tag line data */
  let data = tag_line_data.split(" ").filter(|item| item.to_string() != "".to_string()) /* remove whitespace */
    .map(|item| item.to_string())
    .collect::<Vec<String>>();

  /* handle data absent or bypass */
  if data.len() == 0 {
    println!("No tag data found for script no. {}", i + 1);
    return None;
  }
  if data.iter().nth(0).unwrap() == "!" {
    println!("Bypassing script no. {} (! applied)", i + 1);
    return None;
  }

  /* set output path parts */

  /* get output path parts - break first data item on '/' */
  let mut parts_path = data.iter().nth(0).unwrap().split("/").collect::<Vec<&str>>();
  /* get output filename parts - separate last output path part and break on '.' */
  let parts_filename = parts_path.split_off(parts_path.len() - 1).last().unwrap().split(".").collect::<Vec<&str>>();
  let p_f_len = parts_filename.len();

  /* set as dir either remaining output path parts recombined or default dir */
  let dir = if parts_path.len() > 0 { parts_path.join("/") } else { dir.to_string() };
  /* set as basename either all but last output filename part or src basename */
  let basename = if p_f_len > 1 { parts_filename[0..(p_f_len - 1)].join(".") } else { src.split(".").nth(0).unwrap().to_string() };
  /* set as ext last output filename part */
  let ext = parts_filename.iter().last().unwrap().to_string();

  /* assemble return value */

  /* set as code all lines but tag line, recombined */
  let code = lines.skip(1).collect::<Vec<&str>>().join("\n");
  let path = OutputPath{ dir, basename, ext };
  /* set as prog tag line second item else '?' indicating absence (cf. function exec below) */
  let prog = if data.len() != 1 { data.iter().nth(1).unwrap().to_owned() } else { "?".to_string() };
  /* set as args Vec containing tag line remaining items */
  let args = data.iter().skip(2).map(|arg| arg.to_owned()).collect::<Vec<String>>();

  return Some(Output { code, path, prog, args, i });
}

fn make(dir: String) {

  /* add directory if none */
  fs::create_dir_all(&dir).expect(&format!("create directory '{}'", dir));
}

fn save(path: &String, code: String) {

  /* write script to file */
  fs::write(path, code).expect(&format!("write script to '{}'", path));
}

fn exec(prog: String, args: Vec<String>, path: String, i: usize) {

  /* handle run precluded */
  if prog == "!" { return println!("Not running file no. {} (! applied)", i + 1); }
  if prog == "?" { return println!("Not running file no. {} (no values)", i + 1); }

  /* run script from file */
  process::Command::new(&prog).args(args).arg(path)
    .spawn().expect(&format!("run file with '{}'", prog))
    .wait_with_output().expect(&format!("await output from '{}'", prog));
}

fn apply(output: Option<Output>) {

  /* destructure if some */
  let Output { code, path, prog, args, i } = match output {
    Some(s) => s,
    None    => { return }
  };

  /* destructure and join */
  let OutputPath { dir, basename, ext } = path;
  let path = format!("{}/{}.{}", dir, basename, ext);

  /* perform final tasks */
  make(dir);
  save(&path, code);
  exec(prog, args, path, i);
}

#[cfg(test)]
mod test {

  use::std::collections::HashMap;
  use super::{ CLIOptVal, OutputPath, Output, parse };

  fn get_values_parse() -> (&'static str, &'static str, &'static str, usize, String, Vec<String>, String, OutputPath, HashMap<String, CLIOptVal>) {

    let dir_default_str = "scripts";
    let src_default_str = "src.txt";
    let src_basename_default_str = src_default_str.split(".").nth(0).unwrap();
    let tag_1_default = "#";

    let opts_values_default = HashMap::new();

    /* base test script values */
    let index = 1;
    let ext   = String::from("ext");
    let prog  = String::from("program");
    let args  = Vec::from([String::from("--flag"), String::from("value")]);
    let code  = String::from("//code");

    let output_path = OutputPath {
      dir: String::from(dir_default_str),
      basename: String::from(src_basename_default_str),
      ext
    };

    (dir_default_str, src_default_str, tag_1_default, index, prog, args, code, output_path, opts_values_default)
  }

  #[test]
  fn parse_returns_for_tag_data_full_some_output() {

    let (dir_default_str, src_default_str, tag_1_default, i, prog, args, code, path, opts_values_default) = get_values_parse();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code";

    let expected = Option::Some(Output { code, path, prog, args, i });
    let obtained = parse(script_plus_tag_line_part, tag_1_default, dir_default_str, src_default_str, i, &opts_values_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_label_and_data_full_some_output() {

    let (dir_default_str, src_default_str, tag_1_default, i, prog, args, code, path, opts_values_default) = get_values_parse();
    let script_plus_tag_line_part = " label # ext program --flag value\n\n//code";

    let expected = Option::Some(Output { code, path, prog, args, i });
    let obtained = parse(script_plus_tag_line_part, tag_1_default, dir_default_str, src_default_str, i, &opts_values_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_list_option_none() {

    let (dir_default_str, src_default_str, tag_1_default, i, _, _, _, _, _) = get_values_parse();
    let script_plus_tag_line_part = " ext program --flag value\n\n//code";

    let mut opts_values_default = HashMap::new();
    opts_values_default.insert("is_list".to_string(), CLIOptVal::Bool(true));

    let expected = Option::None;
    let obtained = parse(script_plus_tag_line_part, tag_1_default, dir_default_str, src_default_str, i, &opts_values_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_incl_singlepart_output_basename_some_output() {

    let (dir_default_str, src_default_str, tag_1_default, i, prog, args, code, _, opts_values_default) = get_values_parse();
    let script_plus_tag_line_part = " script.ext program --flag value\n\n//code";

    let dir = String::from(dir_default_str);
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = OutputPath { dir, basename, ext };

    let expected = Option::Some(Output { code, path, prog, args, i });
    let obtained = parse(script_plus_tag_line_part, tag_1_default, dir_default_str, src_default_str, i, &opts_values_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_incl_multipart_output_basename_some_output() {

    let (dir_default_str, src_default_str, tag_1_default, i, prog, args, code, _, opts_values_default) = get_values_parse();
    let script_plus_tag_line_part = " script.suffix1.suffix2.ext program --flag value\n\n//code";

    let dir = String::from(dir_default_str);
    let basename = String::from("script.suffix1.suffix2");
    let ext = String::from("ext");
    let path = OutputPath { dir, basename, ext };

    let expected = Option::Some(Output { code, path, prog, args, i });
    let obtained = parse(script_plus_tag_line_part, tag_1_default, dir_default_str, src_default_str, i, &opts_values_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_incl_output_dir_some_output() {

    let (dir_default_str, src_default_str, tag_1_default, i, prog, args, code, _, opts_values_default) = get_values_parse();
    let script_plus_tag_line_part = " dir/script.ext program --flag value\n\n//code";

    let dir = String::from("dir");
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = OutputPath { dir, basename, ext };

    let expected = Option::Some(Output { code, path, prog, args, i });
    let obtained = parse(script_plus_tag_line_part, tag_1_default, dir_default_str, src_default_str, i, &opts_values_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_minus_cmd_some_output_indicating() {

    let (dir_default_str, src_default_str, tag_1_default, i, _, _, code, path, opts_values_default) = get_values_parse();
    let script_plus_tag_line_part = " ext\n\n//code";

    let expected = Option::Some(Output {
      code, path,
      prog: String::from("?"),
      args: Vec::from([]),
      i
    });

    let obtained = parse(script_plus_tag_line_part, tag_1_default, dir_default_str, src_default_str, i, &opts_values_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_with_bypass_none() {

    let (dir_default_str, src_default_str, tag_1_default, i, _, _, _, _, opts_values_default) = get_values_parse();
    let script_plus_tag_line_part = " ! ext program --flag value\n\n//code";

    let expected = Option::None;
    let obtained = parse(script_plus_tag_line_part, tag_1_default, dir_default_str, src_default_str, i, &opts_values_default);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_absent_none() {

    let (dir_default_str, src_default_str, tag_1_default, i, _, _, _, _, opts_values_default) = get_values_parse();
    let script_plus_tag_line_part = "\n\n//code";

    let expected = Option::None;
    let obtained = parse(script_plus_tag_line_part, tag_1_default, dir_default_str, src_default_str, i, &opts_values_default);

    assert_eq!(expected, obtained);
  }
}
