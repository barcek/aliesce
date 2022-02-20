use std::env;
use std::fs;
use std::process;

#[derive(Debug, PartialEq)]
struct Output {
  code: String,
  path: String,
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

  /* add directory if none */
  fs::create_dir_all(dir).expect(&format!("create directory '{}'", dir));

  /* extract, save and run */
  fs::read_to_string(src).expect(&format!("read source file '{}'", src))
    .split(tag)
    .skip(1) /* omit content preceding initial tag */
    .enumerate() /* yield also index (i) */
    .map(|(i, script)| parse(script, dir, src, i))
    .for_each(apply)
}

fn parse<'a>(script: &'a str, dir: &str, src: &str, i: usize) -> Option<Output> {

  let mut lines = script.lines();

  /* extract data from tag */
  let data = lines.nth(0).unwrap()
    .trim().split(" ").filter(|item| item.to_string() != "".to_string()) /* remove whitespace */
    .map(|item| item.to_string())
    .collect::<Vec<String>>();

  if data.len() == 0 {
    println!("No tag data found for script no. {}", i + 1);
    return None;
  }
  if data.iter().nth(0).unwrap() == "!" {
    println!("Bypassing script no. {} (! applied)", i + 1);
    return None;
  }

  /* get output path parts */
  let parts = data.iter().nth(0).unwrap().split(".").collect::<Vec<&str>>();
  let basename = if parts.len() == 2 { parts.iter().nth(0).unwrap() } else { src.split(".").nth(0).unwrap() };
  let ext = parts.iter().last().unwrap();

  /* assemble return value */
  let code = lines.skip(1).collect::<Vec<&str>>().join("\n");
  let path = format!("{}/{}.{}", dir, basename, ext);
  let prog = if data.len() != 1 { data.iter().nth(1).unwrap().to_owned() } else { "?".to_string() };
  let args = data.iter().skip(2).map(|arg| arg.to_owned()).collect::<Vec<String>>();

  return Some(Output { code, path, prog, args, i });
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

  /* perform final tasks */
  save(&path, code);
  exec(prog, args, path, i);
}

#[cfg(test)]
mod test {

  use super::{ Output, parse };

  fn get_defaults_parse() -> (&'static str, &'static str, usize, String, String) {
    ("scripts", "src.txt", 1, String::from("//code"), String::from("scripts/src.ext"))
  }

  #[test]
  fn parse_returns_for_tag_data_full_some_output() {

    let (dir, src, i, code, path) = get_defaults_parse();
    let script = " ext program --flag value\n\n//code";

    let expected = Option::Some(Output {
      code, path,
      prog: String::from("program"),
      args: Vec::from([String::from("--flag"), String::from("value")]),
      i
    });

    let obtained = parse(script, dir, src, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_plus_output_basename_some_output() {

    let (dir, src, i, code, _) = get_defaults_parse();
    let script = " script.ext program --flag value\n\n//code";

    let expected = Option::Some(Output {
      code,
      path: String::from("scripts/script.ext"),
      prog: String::from("program"),
      args: Vec::from([String::from("--flag"), String::from("value")]),
      i
    });

    let obtained = parse(script, dir, src, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_minus_cmd_some_output_indicating() {

    let (dir, src, i, code, path) = get_defaults_parse();
    let script = " ext\n\n//code";

    let expected = Option::Some(Output {
      code, path,
      prog: String::from("?"),
      args: Vec::from([]),
      i
    });

    let obtained = parse(script, dir, src, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_full_with_bypass_none() {

    let (dir, src, i, _, _) = get_defaults_parse();
    let script = " ! ext program --flag value\n\n//code";

    let expected = Option::None;
    let obtained = parse(script, dir, src, i);

    assert_eq!(expected, obtained);
  }

  #[test]
  fn parse_returns_for_tag_data_absent_none() {

    let (dir, src, i, _, _) = get_defaults_parse();
    let script = "\n\n//code";

    let expected = Option::None;
    let obtained = parse(script, dir, src, i);

    assert_eq!(expected, obtained);
  }
}
