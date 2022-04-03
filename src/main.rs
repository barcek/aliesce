use std::env;
use std::fs;
use std::process;

#[derive(Debug, PartialEq)]
struct Path {
  dir: String,
  basename: String,
  ext: String
}

#[derive(Debug, PartialEq)]
struct Output {
  code: String,
  path: Path,
  prog: String,
  args: Vec<String>,
  i: usize
}

fn main() {

  /* configure */

  let args: Vec<String> = env::args().collect();

  let dir = "scripts";
  let src = if &args.len() > &1 { &args[1] } else { "src.txt" };
  let tag = "###";

  /* implement */

  /* get each script incl. tag line, handle tag line, save and run */
  fs::read_to_string(src).expect(&format!("read source file '{}'", src))
    .split(tag)
    .skip(1) /* omit content preceding initial tag */
    .enumerate() /* yield also index (i) */
    .map(|(i, script_tagged)| parse(script_tagged, dir, src, i))
    .for_each(apply)
}

fn parse<'a>(script_tagged: &'a str, dir: &str, src: &str, i: usize) -> Option<Output> {

  let mut lines = script_tagged.lines();

  /* get data items from tag line */
  let data = lines.nth(0).unwrap()
    .trim().split(" ").filter(|item| item.to_string() != "".to_string()) /* remove whitespace */
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

  /* generate output path */

  /* get output path parts - break first data item on '/' */
  let mut parts_path = data.iter().nth(0).unwrap().split("/").collect::<Vec<&str>>();
  /* get output filename parts - separate last output path part and break on '.' */
  let parts_filename = parts_path.split_off(parts_path.len() - 1).last().unwrap().split(".").collect::<Vec<&str>>();
  let p_f_len = parts_filename.len();

  /* set as dir either remaining output path parts rejoined or default dir */
  let dir = if parts_path.len() > 0 { parts_path.join("/") } else { dir.to_string() };
  /* set as basename either all but last output filename part or src extension */
  let basename = if p_f_len > 1 { parts_filename[0..(p_f_len - 1)].join(".") } else { src.split(".").nth(0).unwrap().to_string() };
  /* set as ext last output filename part */
  let ext = parts_filename.iter().last().unwrap().to_string();

  /* assemble return value */
  let code = lines.skip(1).collect::<Vec<&str>>().join("\n");
  let path = Path{ dir, basename, ext };
  let prog = if data.len() != 1 { data.iter().nth(1).unwrap().to_owned() } else { "?".to_string() };
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
  let Path { dir, basename, ext } = path;
  let path = format!("{}/{}.{}", dir, basename, ext);

  /* perform final tasks */
  make(dir);
  save(&path, code);
  exec(prog, args, path, i);
}

#[cfg(test)]
mod test {

  use super::{ Path, Output, parse };

  fn get_defaults_parse() -> (&'static str, &'static str, usize, String, Path) {
    let dir = String::from("scripts");
    let basename = String::from("src");
    let ext = String::from("ext");
    let path = Path { dir, basename, ext };
    ("scripts", "src.txt", 1, String::from("//code"), path)
  }

  #[test]
  fn parse_returns_for_tag_data_full_some_output() {

    let (dir, src, i, code, path) = get_defaults_parse();
    let script_tagged = " ext program --flag value\n\n//code";

    let expected = Option::Some(Output {
      code, path,
      prog: String::from("program"),
      args: Vec::from([String::from("--flag"), String::from("value")]),
      i
    });

    let obtained = parse(script_tagged, dir, src, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_plus_singlepart_output_basename_some_output() {

    let (dir_default, src, i, code, _) = get_defaults_parse();
    let script_tagged = " script.ext program --flag value\n\n//code";

    let dir = String::from("scripts");
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = Path { dir, basename, ext };

    let expected = Option::Some(Output {
      code,
      path: path,
      prog: String::from("program"),
      args: Vec::from([String::from("--flag"), String::from("value")]),
      i
    });

    let obtained = parse(script_tagged, dir_default, src, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_plus_multipart_output_basename_some_output() {

    let (dir_default, src, i, code, _) = get_defaults_parse();
    let script_tagged = " script.suffix1.suffix2.ext program --flag value\n\n//code";

    let dir = String::from("scripts");
    let basename = String::from("script.suffix1.suffix2");
    let ext = String::from("ext");
    let path = Path { dir, basename, ext };

    let expected = Option::Some(Output {
      code,
      path: path,
      prog: String::from("program"),
      args: Vec::from([String::from("--flag"), String::from("value")]),
      i
    });

    let obtained = parse(script_tagged, dir_default, src, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_plus_output_dir_some_output() {

    let (dir_default, src, i, code, _) = get_defaults_parse();
    let script_tagged = " dir/script.ext program --flag value\n\n//code";

    let dir = String::from("dir");
    let basename = String::from("script");
    let ext = String::from("ext");
    let path = Path { dir, basename, ext };

    let expected = Option::Some(Output {
      code,
      path: path,
      prog: String::from("program"),
      args: Vec::from([String::from("--flag"), String::from("value")]),
      i
    });

    let obtained = parse(script_tagged, dir_default, src, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_minus_cmd_some_output_indicating() {

    let (dir, src, i, code, path) = get_defaults_parse();
    let script_tagged = " ext\n\n//code";

    let expected = Option::Some(Output {
      code, path,
      prog: String::from("?"),
      args: Vec::from([]),
      i
    });

    let obtained = parse(script_tagged, dir, src, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_with_bypass_none() {

    let (dir, src, i, _, _) = get_defaults_parse();
    let script_tagged = " ! ext program --flag value\n\n//code";

    let expected = Option::None;
    let obtained = parse(script_tagged, dir, src, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_absent_none() {

    let (dir, src, i, _, _) = get_defaults_parse();
    let script_tagged = "\n\n//code";

    let expected = Option::None;
    let obtained = parse(script_tagged, dir, src, i);

    assert_eq!(expected, obtained);
  }
}
