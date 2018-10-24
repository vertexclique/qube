extern crate qube;
extern crate tokio;
extern crate futures;
extern crate reqwest;
extern crate rand;
extern crate colored;

use qube::prelude::*;
use qube::errors::*;
use std::env;
use std::io::{self, Write};
use tokio::run;
use reqwest::async::{Client, Decoder};
use futures::{Future, Stream};
use rand::Rng;
use colored::*;
use rand::thread_rng;
use std::str::*;

fn run_list_pods() -> Result<i32> {
    // filename is set to $KUBECONFIG if the env var is available.
    // Otherwise it falls back to "admin.conf".
    let filename = env::var("KUBECONFIG").ok();
    let filename = filename
        .as_ref()
        .map(String::as_str)
        .and_then(|s| if s.is_empty() { None } else { Some(s) })
        .unwrap_or("~/.kube/config");
    let kube = Kubernetes::load_conf_with_ctx(filename, "kluster")?;

    let colors: [String; 8] = [
        String::from("blue"),
        String::from("red"),
        String::from("green"),
        String::from("yellow"),
        String::from("cyan"),
        String::from("purple"),
        String::from("magenta"),
        String::from("white")
    ];

    if kube.healthy()? {
        let podname = "test-pod";
        let d = kube.pods().namespace("default").logs().fetch_container_future(podname, "test-container");
        let runfut = d.unwrap()
            .send()
            .and_then(move |mut res| {
                // println!("{}", res.status());

                let name = format!("{} ::⇒ ", podname);
                let color = thread_rng().choose(&colors).unwrap();
                let resname = name.color(color.to_string());

                println!("{:?}", resname.as_bytes());

                res
                    .into_body()
                    .for_each(move |chunk| {
                        let stdout = io::stdout();
                        let mut handle = stdout.lock();

                        let data = format!("{}{}\n", resname, from_utf8(&chunk).unwrap().trim_right());

                        handle
                            .write_all(&data.as_bytes())
                            .map_err(|e| {
                                panic!("stdout expected to be open, error={}", e)
                            })
                    })
            })
            .map_err(|err| println!("request error: {}", err));

        run(runfut);
    }

    Ok(0)
}

fn main() {
    match run_list_pods() {
        Ok(n) => println!("Success error code is {}", n),
        Err(e) => println!("Error: {}", e),
    }
}
