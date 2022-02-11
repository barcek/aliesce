use std::env;
use std::fs;
use std::process;

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

fn parse<'a>(script: &'a str, dir: &str, src: &str, i: usize) -> Option<(String, String, Vec<String>, String, usize)> {

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
  let basename = src.split(".").nth(0).unwrap();
  let ext = data.iter().nth(0).unwrap();

  /* assemble return value */
  let code = lines.skip(1).collect::<Vec<&str>>().join("\n");
  let prog = if data.len() != 1 { data.iter().nth(1).unwrap().to_owned() } else { "?".to_string() };
  let args = data.iter().skip(2).map(|arg| arg.to_owned()).collect::<Vec<String>>();
  let path = format!("{}/{}.{}", dir, basename, ext);

  return Some((code, prog, args, path, i));
}

fn save(path: &String, code: String) {

  /* write script to file */
  fs::write(path, code).expect(&format!("write script to '{}'", path));
}

fn exec(prog: String, args: Vec<String>, path: String, i: usize) {

  if prog == "!" {
    println!("Not running file no. {} (! applied)", i + 1);
    return
  }

  if prog == "?" {
    println!("Not running file no. {} (no values)", i + 1);
    return
  }

  /* run script from file */
  process::Command::new(&prog).args(args).arg(path)
    .spawn().expect(&format!("run file with '{}'", prog))
    .wait_with_output().expect(&format!("await output from '{}'", prog));
}

fn apply(strs: Option<(String, String, Vec<String>, String, usize)>) {

  /* destructure if tuple */
  let (code, prog, args, path, i) = match strs {
    Some(t) => t,
    None    => { return }
  };

  /* perform final tasks */
  save(&path, code);
  exec(prog, args, path, i);
}
