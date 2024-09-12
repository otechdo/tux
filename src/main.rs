use chrono::Local;
use inquire::{Confirm, Select, Text};
use is_executable::IsExecutable;
use std::env::{current_dir, set_current_dir};
use std::path::{Path, MAIN_SEPARATOR_STR};
use std::process::{ExitStatus, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};
use std::{
    fs::read_to_string,
    io::{Error, ErrorKind},
    process::Command,
};
use toml::Value;

fn docker(verb: &str, args: &[&str], path: &str) -> Result<(), Error> {
    if let Ok(mut child) = Command::new("docker")
        .arg(verb)
        .args(args)
        .current_dir(path)
        .spawn()
    {
        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
            return Err(Error::new(
                ErrorKind::Other,
                "Docker exited with status no 0",
            ));
        }
    }
    Err(Error::new(ErrorKind::NotFound, "docker not found"))
}

fn dirs() -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    let walk = ignore::WalkBuilder::new(".")
        .standard_filters(true)
        .threads(4)
        .add_custom_ignore_filename("ignore.ji")
        .filter_entry(move |e| e.path().is_dir())
        .hidden(false)
        .build();
    for entry in walk.flatten() {
        if entry.file_type().unwrap().is_dir() {
            dirs.push(entry.path().to_str().unwrap().to_string());
        }
    }
    dirs
}

fn jump() {
    loop {
        let jump = Select::new("Select a folder for jump : ", dirs())
            .prompt()
            .unwrap();
        assert!(cd(jump.as_str()).is_ok());
        if Confirm::new("jump on an another directory ? ")
            .with_default(false)
            .prompt()
            .unwrap()
            .eq(&true)
        {
            continue;
        }
        break;
    }
}
fn cd(dir: &str) -> std::io::Result<()> {
    set_current_dir(dir)
}
fn ssh_run(args: &[&str], user: &str, ip: &str) -> Result<(), Error> {
    if let Ok(mut cmd) = Command::new("ssh")
        .arg(format!("{user}@{ip}").as_str())
        .args(args)
        .spawn()
    {
        return match cmd.wait() {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        };
    }
    Err(Error::new(ErrorKind::NotFound, "ssh not found"))
}

fn list_networks() -> Result<(), Error> {
    docker("network", &["ls"], "/tmp")
}
fn upload_image(user: &str, ip: &str, s: &str, port: &str) -> Result<(), Error> {
    if let Ok(mut cmd) = Command::new("rsync")
        .arg("-a")
        .arg("-z")
        .arg("-e")
        .arg(format!("ssh -p {port}").as_str())
        .arg(format!("./containers/{s}/").as_str())
        .arg(format!("{user}@{ip}:{s}").as_str())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        return match cmd.wait() {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        };
    }
    Err(Error::new(ErrorKind::NotFound, "rsync not found"))
}

fn clear() -> Result<(), Error> {
    if let Ok(mut child) = Command::new("clear").spawn() {
        assert!(child.wait().is_ok());
        return Ok(());
    }
    Err(Error::new(ErrorKind::NotFound, "clear failed"))
}
fn login() -> Result<(), Error> {
    let username = Text::new("Please enter your docker username : ")
        .with_default(env!("USER"))
        .prompt()
        .unwrap_or_default();
    if docker("login", &["-u", username.as_str()], "/tmp").is_ok() {
        log(format!("Logged as {username}").as_str());
        return Ok(());
    }
    Err(Error::new(ErrorKind::NotFound, "docker username not found"))
}

fn logout() -> Result<(), Error> {
    if docker("logout", &[], "/tmp").is_ok() {
        log("Disconnected successfully");
        return Ok(());
    }
    Err(Error::new(ErrorKind::NotFound, "docker logout not found"))
}

fn servers() -> Result<Vec<String>, Error> {
    let mut servers: Vec<String> = Vec::new();
    if let Ok(config) = configuration() {
        if let Some(tables) = config.as_table() {
            for (server_name, _) in tables {
                if server_name.ne("local") {
                    servers.push(server_name.to_string());
                }
            }
            return Ok(servers);
        }
        return Err(Error::new(ErrorKind::InvalidData, "must be a table"));
    }
    Err(Error::new(ErrorKind::NotFound, "Missing configuration"))
}

fn ssh() -> Result<ExitStatus, Error> {
    let server = Text::new("Please enter the server to connect :")
        .prompt()
        .unwrap_or_default();
    let user = Text::new("Please enter the username :")
        .with_default("root")
        .prompt()
        .unwrap_or_default();
    let port = Text::new("Please enter the ssh port :")
        .with_default("22")
        .prompt()
        .unwrap_or_default();

    if let Ok(ssh) = cmd(
        "ssh",
        &[
            format!("-p {port}").as_str(),
            format!("{user}@{server}").as_str(),
        ],
    ) {
        return Ok(ssh);
    }
    Err(Error::new(ErrorKind::NotFound, "ssh not found"))
}
fn configuration() -> Result<Value, Error> {
    if let Ok(conf) = read_to_string("docks.toml") {
        if let Ok(config) = toml::from_str::<Value>(&conf) {
            return Ok(config);
        }
        return Err(Error::last_os_error());
    }
    Err(Error::last_os_error())
}
fn log(message: &str) {
    println!(
        "{}",
        format!("\x1b[1;32m ✔\x1b[0;37m {message}\x1b[0m").as_str()
    );
}
fn cmd(program: &str, args: &[&str]) -> Result<ExitStatus, Error> {
    if let Ok(mut child) = Command::new(program).args(args).current_dir(".").spawn() {
        return child.wait();
    }
    Err(Error::new(ErrorKind::NotFound, "program not found"))
}
fn check_connexion(ip: &str, port: &str) -> Result<(), Error> {
    log(format!("Checking the ssh connexion on {ip}").as_str());
    sleep(Duration::from_secs(1));
    if let Ok(status) = cmd("ncat", &["-z", ip, port]) {
        if status.success() {
            log(format!("Can communicate to the {ip} server").as_str());
            return Ok(());
        }
        log(format!("Cannot communicate to the {ip} server").as_str());
        return Err(Error::new(ErrorKind::NotFound, "ncat connexion failed"));
    }
    Err(Error::new(ErrorKind::NotFound, "ncat not founded"))
}
fn running(user: &str, ip: &str) -> Result<(), Error> {
    ssh_run(&["docker", "ps"], user, ip)
}
fn ps() -> Result<(), Error> {
    docker("ps", &["-a"], "/tmp")
}

fn build(tag: &str) -> Result<(), Error> {
    if Path::new("Dockerfile").is_file() {
        return docker("buildx", &["build", "-t", tag, "."], ".");
    }
    Err(Error::new(ErrorKind::NotFound, "Dockerfile not found"))
}
fn publish(container: &str) -> Result<(), Error> {
    docker("push", &[container], "/tmp")
}
fn deploy_local() -> Result<(), Error> {
    if let Ok(docks) = configuration() {
        if let Some(table) = docks.as_table() {
            if let Some(local) = table.get("local") {
                let containers = local.get("containers").unwrap().as_array().unwrap();
                for container in containers {
                    let x = format!("./containers/{}", container.as_str().unwrap_or_default());
                    let xf = format!(
                        "./containers/{}/compose.yaml",
                        container.as_str().unwrap_or_default()
                    );
                    if Path::new(x.as_str()).is_dir() && Path::new(&xf).is_file() {
                        assert!(
                            docker("compose", &["down"], x.as_str()).is_ok(),
                            "fail to stop container"
                        );
                        assert!(
                            docker("compose", &["pull"], x.as_str()).is_ok(),
                            "fail to update container"
                        );
                        assert!(
                            docker("compose", &["up", "-d"], x.as_str()).is_ok(),
                            "fail to start container"
                        );
                    } else {
                        return Err(Error::new(
                            ErrorKind::NotFound,
                            "docker container is not a dir",
                        ));
                    }
                }
                return Ok(());
            }
            return Err(Error::new(ErrorKind::NotFound, "missing local id"));
        }
        return Err(Error::new(ErrorKind::NotFound, "docker config not valid"));
    }
    Err(Error::new(
        ErrorKind::NotFound,
        "docks.toml config not found",
    ))
}
fn deploy_to_remote() -> Result<(), Error> {
    if let Ok(docks) = configuration() {
        if let Ok(servers) = servers() {
            let park = servers.len();
            if park.gt(&1) {
                log(format!("Deploying docker containers on {park} servers").as_str());
            } else {
                log(format!("Deploying docker containers on {park} server").as_str());
            }
            for server in &servers {
                if let Some(table) = docks.as_table() {
                    if let Some(config) = table.get(server.as_str()) {
                        let username = config.get("username").unwrap().as_str().unwrap();
                        let port = config.get("port").unwrap().as_str().unwrap();
                        let ip = config.get("ip").unwrap().as_str().unwrap();
                        let containers = config.get("containers").unwrap().as_array().unwrap();
                        assert!(
                            check_connexion(ip, port).is_ok(),
                            "Cannot deploy containers"
                        );
                        for container in containers {
                            let image = container.as_str().unwrap();
                            log(
                                format!("Deploying {image} docker container on {server} server")
                                    .as_str(),
                            );
                            assert!(upload_image(username, ip, image, port).is_ok());
                            log(format!(
                                "The {image} has been deployed successfully on the {server} server"
                            )
                            .as_str());
                            log(
                                format!("Stopping {image} before update on the {server} server")
                                    .as_str(),
                            );
                            assert!(
                                ssh_run(
                                    &["docker", "compose", "--project-directory", image, "down",],
                                    username,
                                    ip,
                                )
                                .is_ok(),
                                "Failed to stop container"
                            );
                            log(
                                format!("Updating the {image} container on the {server} server")
                                    .as_str(),
                            );
                            assert!(
                                ssh_run(
                                    &["docker", "compose", "--project-directory", image, "pull"],
                                    username,
                                    ip
                                )
                                .is_ok(),
                                "Failed to update container"
                            );
                            log(format!("The {image} container has been updated successfully on the {server} server").as_str());
                            log(format!(
                                "Restarting the {image} after upgrade on the {server} server"
                            )
                            .as_str());
                            assert!(
                                ssh_run(
                                    &[
                                        "docker",
                                        "compose",
                                        "--project-directory",
                                        image,
                                        "up",
                                        "-d",
                                    ],
                                    username,
                                    ip
                                )
                                .is_ok(),
                                "Failed to start the container"
                            );
                            log(format!(
                                "The {image} has been restarted successfully on the {server} server"
                            )
                            .as_str());
                        }
                    }
                }
            }
        }
        return Ok(());
    }
    Err(Error::new(ErrorKind::NotFound, "docks.toml not found"))
}

fn deploy() {
    let now = Instant::now();
    let date = Local::now();
    log(format!("Starting deployment at {date}").as_str());
    assert!(deploy_local().is_ok());
    assert!(deploy_to_remote().is_ok());
    log(format!("The deployment take {} secs", now.elapsed().as_secs()).as_str());
}

fn editor() -> Result<(), Error> {
    if let Ok(mut child) = Command::new("ranger").current_dir(".").spawn() {
        assert!(child.wait().is_ok());
        return Ok(());
    }
    Err(Error::new(ErrorKind::NotFound, "Failed to run ranger"))
}
fn dock_running() -> Result<(), Error> {
    let tux = configuration()?;
    let servers = servers()?;
    for server in &servers {
        if let Some(username) = tux
            .get(server.as_str())
            .and_then(|value: &Value| value.get("username"))
        {
            if let Some(ip) = tux
                .get(server.as_str())
                .and_then(|value: &Value| value.get("ip"))
            {
                assert!(running(
                    username.as_str().unwrap_or_default(),
                    ip.as_str().unwrap_or_default(),
                )
                .is_ok());
            }
        }
    }
    Ok(())
}

fn list() {
    assert!(list_images().is_ok());
    assert!(list_networks().is_ok());
    assert!(list_volumes().is_ok());
}

fn remove() -> Result<(), Error> {
    loop {
        assert!(clear().is_ok());
        assert!(list_images().is_ok());
        let image = Text::new("please enter the name or the id of the image to remove : ")
            .prompt()
            .unwrap_or_default();
        if docker("image", &["rm", image.as_str()], "/tmp").is_ok() {
            if Confirm::new("remove an other image ? :")
                .with_default(false)
                .prompt()
                .unwrap()
                .eq(&true)
            {
                continue;
            }
            break;
        }
        return Err(Error::new(ErrorKind::NotFound, "Failed to remove image"));
    }
    Ok(())
}
fn stop() -> Result<(), Error> {
    loop {
        assert!(clear().is_ok());
        assert!(ps().is_ok());
        let image = Text::new("please enter the name or the id of the container to stop : ")
            .prompt()
            .unwrap_or_default();
        if docker("stop", &[image.as_str()], "/tmp").is_ok() {
            if Confirm::new("stop an other container ? :")
                .with_default(false)
                .prompt()
                .unwrap()
                .eq(&true)
            {
                continue;
            }
            break;
        }
        return Err(Error::new(
            ErrorKind::NotFound,
            "Failed to stop the container",
        ));
    }
    Ok(())
}
fn start() -> Result<(), Error> {
    loop {
        assert!(clear().is_ok());
        assert!(list_images().is_ok());
        let image = Text::new("please enter the name or the id of the image to run : ")
            .prompt()
            .unwrap();
        let host_port = Text::new("please enter the host port  : ")
            .prompt()
            .unwrap();
        let container_port = Text::new("please enter the container port  : ")
            .prompt()
            .unwrap();
        if docker(
            "run",
            &[
                "-d",
                "-p",
                format!("{host_port}:{container_port}").as_str(),
                image.as_str(),
            ],
            "/tmp",
        )
        .is_ok()
        {
            assert!(ps().is_ok());
            if Confirm::new("run an other container ? :")
                .with_default(false)
                .prompt()
                .unwrap()
                .eq(&true)
            {
                continue;
            }
            break;
        }
        return Err(Error::new(
            ErrorKind::NotFound,
            "Failed to run the container",
        ));
    }
    Ok(())
}

fn restart() -> Result<(), Error> {
    loop {
        assert!(clear().is_ok());
        assert!(ps().is_ok());
        let image = Text::new("please enter the name or the id of the image to restart : ")
            .prompt()
            .unwrap();

        if docker("restart", &[image.as_str()], "/tmp").is_ok() {
            assert!(ps().is_ok());
            if Confirm::new("restart an other container ? :")
                .with_default(false)
                .prompt()
                .unwrap()
                .eq(&true)
            {
                continue;
            }
            break;
        }
        return Err(Error::new(
            ErrorKind::NotFound,
            "Failed to restart the container",
        ));
    }
    Ok(())
}

fn touch() -> Result<(), Error> {
    if Confirm::new("create a Dockerfile")
        .with_default(false)
        .prompt()
        .unwrap()
        .eq(&true)
    {
        if let Ok(mut child) = Command::new("touch")
            .arg("Dockerfile")
            .current_dir(".")
            .spawn()
        {
            if child.wait().is_ok() {
                return Ok(());
            }
            return Err(Error::new(
                ErrorKind::NotFound,
                "Failed to create Dockerfile",
            ));
        }
        return Err(Error::new(ErrorKind::NotFound, "touch not found"));
    }
    if let Ok(mut child) = Command::new("touch")
        .arg("compose.yaml")
        .current_dir(".")
        .spawn()
    {
        if child.wait().is_ok() {
            return Ok(());
        }
        return Err(Error::new(
            ErrorKind::NotFound,
            "Failed to create compose.yaml",
        ));
    }
    Err(Error::new(ErrorKind::NotFound, "touch not found"))
}
fn pull() {
    loop {
        assert!(clear().is_ok());
        assert!(list_images().is_ok());
        let image = Text::new("please enter the name or the id of the image to pull : ")
            .prompt()
            .unwrap();

        let tag = Text::new("please enter the image tag to pull : ")
            .with_default("latest")
            .prompt()
            .unwrap();
        if docker(
            "pull",
            &[format!("{}:{}", image.as_str(), tag.as_str()).as_str()],
            "/tmp",
        )
        .is_ok()
        {
            assert!(ps().is_ok());
            if Confirm::new("pull an other image ? :")
                .with_default(false)
                .prompt()
                .unwrap()
                .eq(&true)
            {
                continue;
            }
            break;
        }
    }
}

fn list_volumes() -> Result<(), Error> {
    docker("volume", &["ls"], "/tmp")
}
fn list_images() -> Result<(), Error> {
    docker("images", &[], "/tmp")
}
fn main() {
    assert!(clear().is_ok());
    if Path::new("/usr/bin/ranger").is_executable() {
        loop {
            let project = current_dir().map_or_else(
                |_| String::from("."),
                |d| {
                    let parts = d
                        .to_str()
                        .unwrap()
                        .split(MAIN_SEPARATOR_STR)
                        .collect::<Vec<&str>>();
                    parts
                        .last()
                        .map_or_else(|| String::from("unknown"), |p| (*p).to_string())
                },
            );
            let tasks = vec![
                "build", "clear", "check", "cd", "deploy", "exit", "editor", "ls", "list", "login",
                "logout", "push", "pull", "ps", "rm", "start", "restart", "stop", "ssh", "touch",
                "ps",
            ];

            let selected = Select::new(
                format!("\x1b[1;34mWhat you want to do in the \x1b[1;36m{project}\x1b[1;34m project :\x1b[0m").as_str(),
                tasks,
            )
                .prompt()
                .unwrap_or_default();
            match selected {
                "login" => assert!(login().is_ok()),
                "logout" => assert!(logout().is_ok()),
                "clear" => assert!(clear().is_ok()),
                "deploy" => deploy(),
                "check" => assert!(dock_running().is_ok()),
                "cd" => jump(),
                "ssh" => assert!(ssh().is_ok()),
                "stop" => assert!(stop().is_ok()),
                "logs" => logs(),
                "list" => list(),
                "ls" => ls(),
                "start" => assert!(start().is_ok()),
                "restart" => assert!(restart().is_ok()),
                "rm" => assert!(remove().is_ok()),
                "touch" => assert!(touch().is_ok()),
                "ps" => assert!(ps().is_ok()),
                "pull" => pull(),
                "build" => {
                    let tag = Text::new("Please enter the tag for the image :")
                        .prompt()
                        .unwrap_or_default();
                    assert!(build(tag.as_str()).is_ok());
                }
                "push" => {
                    let image = Text::new("Please enter the image to publish :")
                        .prompt()
                        .unwrap_or_default();
                    assert!(publish(image.as_str()).is_ok());
                }
                "editor" => assert!(editor().is_ok()),
                "exit" => break,
                _ => continue,
            }
        }
    } else {
        println!("Ranger not found");
    }
    println!("Bye");
}

fn ls() {
    if let Ok(mut child) = Command::new("eza")
        .arg("--git")
        .arg("--gitignore")
        .arg("--tree")
        .arg("--level")
        .arg("4")
        .current_dir(".")
        .spawn()
    {
        assert!(child.wait().is_ok());
    }
}

fn logs() {
    loop {
        assert!(clear().is_ok());
        assert!(list_images().is_ok());
        let image = Text::new("please enter the name or the id of the image to show logs : ")
            .prompt()
            .unwrap();

        if docker("logs", &[image.as_str()], "/tmp").is_ok() {
            assert!(ps().is_ok());
            if Confirm::new("show logs of another image ? :")
                .with_default(false)
                .prompt()
                .unwrap()
                .eq(&true)
            {
                continue;
            }
            break;
        }
    }
}
