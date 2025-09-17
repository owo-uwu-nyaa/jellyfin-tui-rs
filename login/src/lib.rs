use std::{
    borrow::Cow,
    fs::{OpenOptions, create_dir_all},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    pin::pin,
};

use color_eyre::eyre::{Context, OptionExt, Report, Result, eyre};
use futures_util::StreamExt;
use jellyfin::{Auth, ClientInfo, JellyfinClient, NoAuth};
use jellyfin_tui_core::{
    config::Config,
    keybinds::{Keybinds, LoadingCommand, LoginInfoCommand},
};
use keybinds::{KeybindEvent, KeybindEventStream, KeybindEvents};
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, BorderType, Padding, Paragraph, Wrap},
};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, query, query_scalar};
use tracing::{error, info, instrument};

#[derive(Debug, Deserialize, Serialize)]
struct LoginInfo {
    server_url: String,
    username: String,
    password: String,
    password_cmd: Option<Vec<String>>
}

enum LoginSelection {
    Server,
    Username,
    Password,
    Retry,
}

#[instrument(skip_all)]
async fn get_login_info(
    term: &mut DefaultTerminal,
    info: &mut LoginInfo,
    changed: &mut bool,
    error: Report,
    events: &mut KeybindEvents,
    keybinds: &Keybinds,
) -> Result<bool> {
    let mut selection = if info.server_url.is_empty() {
        LoginSelection::Server
    } else {
        LoginSelection::Password
    };
    let error = Paragraph::new(error.to_string())
        .block(Block::bordered().border_style(Color::Red))
        .wrap(Wrap::default());
    let normal_block = Block::bordered();
    let current_block = Block::bordered().border_type(ratatui::widgets::BorderType::Double);
    let outer_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .padding(Padding::uniform(4))
        .title("Enter Jellyfin Server / Login Information");
    let mut events = KeybindEventStream::new(events, keybinds.login_info.clone());
    loop {
        term.draw(|frame| {
            let server = Paragraph::new(info.server_url.as_str()).block(
                if let LoginSelection::Server = selection {
                    current_block.clone()
                } else {
                    normal_block.clone()
                }
                .title("Jellyfin URL"),
            );
            let username = Paragraph::new(info.username.as_str()).block(
                if let LoginSelection::Username = selection {
                    current_block.clone()
                } else {
                    normal_block.clone()
                }
                .title("Username"),
            );
            let password = Paragraph::new(
                Text::from(if info.password_cmd.is_some() {"from command"} else if info.password.is_empty() {
                    ""
                } else {
                    "<hidden>"
                })
                .style(Style::default().add_modifier(Modifier::HIDDEN)),
            )
            .block(
                if let LoginSelection::Password = selection {
                    current_block.clone()
                } else {
                    normal_block.clone()
                }
                .title("Password"),
            );
            let outer_area = events.inner(frame.area());
            let button =
                Paragraph::new("Connect").block(if let LoginSelection::Retry = selection {
                    current_block.clone()
                } else {
                    Block::bordered().border_type(BorderType::Thick)
                });

            let [layout_s, layout_u, layout_p, layout_b, layout_e] = Layout::vertical([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(3),
            ])
            .vertical_margin(1)
            .areas(outer_block.inner(outer_area));
            frame.render_widget(&outer_block, outer_area);
            frame.render_widget(server, layout_s);
            frame.render_widget(username, layout_u);
            frame.render_widget(password, layout_p);
            frame.render_widget(button, layout_b);
            frame.render_widget(&error, layout_e);
            frame.render_widget(&mut events, frame.area());
        })?;
        events.set_text_input(!matches!(selection, LoginSelection::Retry));
        match events.next().await {
            Some(Ok(KeybindEvent::Command(LoginInfoCommand::Delete))) => match selection {
                LoginSelection::Server => {
                    info.server_url.pop();
                    *changed = true;
                }
                LoginSelection::Username => {
                    info.username.pop();
                    *changed = true;
                }
                LoginSelection::Password => {
                    info.password.pop();
                    *changed = true;
                }
                LoginSelection::Retry => {}
            },
            Some(Ok(KeybindEvent::Command(LoginInfoCommand::Submit))) => break Ok(true),
            Some(Ok(KeybindEvent::Command(LoginInfoCommand::Prev))) => {
                selection = match selection {
                    LoginSelection::Server => LoginSelection::Retry,
                    LoginSelection::Username => LoginSelection::Server,
                    LoginSelection::Password => LoginSelection::Username,
                    LoginSelection::Retry => LoginSelection::Password,
                }
            }
            Some(Ok(KeybindEvent::Command(LoginInfoCommand::Next))) => {
                selection = match selection {
                    LoginSelection::Server => LoginSelection::Username,
                    LoginSelection::Username => LoginSelection::Password,
                    LoginSelection::Password => LoginSelection::Retry,
                    LoginSelection::Retry => LoginSelection::Server,
                }
            }
            Some(Ok(KeybindEvent::Command(LoginInfoCommand::Quit))) => break Ok(false),
            Some(Ok(KeybindEvent::Text(text))) => {
                let dest = match selection {
                    LoginSelection::Server => &mut info.server_url,
                    LoginSelection::Username => &mut info.username,
                    LoginSelection::Password => &mut info.password,
                    LoginSelection::Retry => {
                        unreachable!("selecting reply should disable text input")
                    }
                };
                match text {
                    keybinds::Text::Char(c) => dest.push(c),
                    keybinds::Text::Str(s) => dest.push_str(&s),
                }
                *changed = true;
            }
            Some(Ok(KeybindEvent::Render)) => {}
            Some(Err(e)) => break Err(e).context("receiving terminal events"),
            None => break Ok(false),
        }
    }
}

#[instrument(skip_all)]
pub async fn login(
    term: &mut DefaultTerminal,
    config: &Config,
    events: &mut KeybindEvents,
    cache: &SqlitePool,
) -> Result<Option<JellyfinClient<Auth>>> {
    let mut login_info: LoginInfo;
    let mut error: Option<Report>;
    let connect_msg = Paragraph::new("Connecting to Server")
        .centered()
        .block(Block::bordered());
    match std::fs::read_to_string(&config.login_file)
        .context("reading login info file")
        .and_then(|config| toml::from_str::<LoginInfo>(&config).context("parsing login info"))
    {
        Ok(info) => {
            login_info = info;
            error = None;
        }
        Err(e) => {
            login_info = LoginInfo {
                server_url: String::new(),
                username: String::new(),
                password: String::new(),
                password_cmd: None,
            };
            error = Some(e);
        }
    }
    let mut info_changed = false;
    let device_name: Cow<'static, str> = whoami::fallible::hostname()
        .ok()
        .map(|v| v.into())
        .unwrap_or_else(|| "unknown".into());
    let client = 'connect: loop {
        if let Some(e) = error.take() {
            error!("Error logging in: {e:?}");
            if !get_login_info(
                term,
                &mut login_info,
                &mut info_changed,
                e,
                events,
                &config.keybinds,
            )
            .await
            .context("getting login information")?
            {
                return Ok(None);
            }
        }
        if login_info.server_url.is_empty() {
            error = Some(eyre!("Server URI is empty"));
            continue;
        }

        let client = match JellyfinClient::<NoAuth>::new(
            &login_info.server_url,
            ClientInfo {
                name: "jellyfin-tui-rs".into(),
                version: "0.1".into(),
            },
            device_name.clone(),
        ) {
            Ok(client) => client,
            Err(e) => {
                error = Some(e);
                continue;
            }
        };
        let mut auth_request = pin!(jellyfin_login(
            client,
            cache,
            &login_info.username,
            &login_info.password,
            login_info.password_cmd.as_deref()
        ));

        let mut events = KeybindEventStream::new(events, config.keybinds.fetch.clone());
        loop {
            term.draw(|frame| frame.render_widget(&connect_msg, frame.area()))
                .context("rendering ui")?;
            tokio::select! {
                event = events.next() => {
                    match event {
                        Some(Ok(KeybindEvent::Command(LoadingCommand::Quit)))|None => return Ok(None),
                        Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
                        Some(Ok(KeybindEvent::Render)) => continue,
                        Some(Err(e)) => return Err(e).context("Error getting key events from terminal"),
                    }
                }
                request = &mut auth_request => {
                    match request {
                        Ok(client) => break 'connect client,
                        Err((_,e)) => {
                            error = Some(e.wrap_err("logging in"));
                            break
                        },
                    }
                }
            };
        }
    };
    if info_changed {
        create_dir_all(
            config
                .login_file
                .parent()
                .ok_or_eyre("login info path has no parent")?,
        )
        .context("creating login info parent dir")?;
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o0600)
            .open(&config.login_file)
            .context("opening login info")?
            .write_all(
                toml::to_string_pretty(&login_info)
                    .context("serializing login info")?
                    .as_bytes(),
            )
            .context("writing out new login info")?;
    }
    Ok(Some(client))
}

async fn get_password_from_cmd(cmd: &[String])-> Result<String>{
    let mut command = if let Some(cmd) = cmd.first(){tokio::process::Command::new(cmd)} else {return Err(eyre!("Password cmd is empty"))};
    for arg in cmd[1..].iter(){
        command.arg(arg);
    }
    let output = command.kill_on_drop(true).output().await.context("Executing password cmd failed")?;
    if output.status.success(){
        Ok(String::from_utf8(output.stdout).context("password cmd output is not utf-8")?.trim().to_string())
    }else{
        Err(eyre!("command failed with:\n{}",String::from_utf8(output.stderr).context("password cmd error output is not utf-8")?))
    }
}

async fn jellyfin_login(
    mut client: JellyfinClient<NoAuth>,
    cache: &SqlitePool,
    username: &str,
    password: &str,
    password_cmd: Option<&[String]>
) -> std::result::Result<JellyfinClient<Auth>, (JellyfinClient<NoAuth>, Report)> {
    let device_name = client.get_device_name();
    let client_name = client.get_client_info().name.as_ref();
    let client_version = client.get_client_info().version.as_ref();
    match query_scalar!("select access_token from creds where device_name = ? and client_name = ? and client_version = ? and user_name = ?",
                        device_name,
                        client_name,
                        client_version,
                        username
    ).fetch_optional(cache).await{
        Ok(None) => {}
        Err(e) => return Err((client,e.into())),
        Ok(Some(access_token)) => {
            info!("testing cached credentials");
            match client.auth_key(access_token, username).get_self().await{
                Ok(client) => {
                    info!("credentials valid");
                    return Ok(client)
                },
                Err((c,e)) => {
                    error!("Error getting self from server: {e:?}");
                    client=c.without_auth();
                    let device_name = client.get_device_name();
                    let client_name = client.get_client_info().name.as_ref();
                    let client_version = client.get_client_info().version.as_ref();
                    match query!("delete from creds where device_name = ? and client_name = ? and client_version = ? and user_name = ?",
                                 device_name,
                                 client_name,
                                 client_version,
                                 username
                    ).execute(cache).await{
                        Ok(_)=>{},
                        Err(e) => {
                            return Err((client,e.into()))
                        }
                    }
                }
            }
        }
    }
    info!("connecting to server");
    let password = if let Some(cmd) = password_cmd{
        match get_password_from_cmd(cmd).await{
            Ok(v) => v,
            Err(e) => return Err((client, e))
        }
    } else{
        password.to_string()
    };
    let client = match client.auth_user_name(username, password).await {
        Ok(v) => v,
        Err((client, e)) => return Err((client, e)),
    };
    let device_name = client.get_device_name();
    let client_name = client.get_client_info().name.as_ref();
    let client_version = client.get_client_info().version.as_ref();
    let access_token = client.get_auth().access_token.as_str();
    match query!("insert into creds (device_name, client_name, client_version, user_name, access_token) values (?, ?, ?, ?, ?)",
                 device_name,
                 client_name,
                 client_version,
                 username,
                 access_token,
    ).execute(cache).await{
        Ok(_)=> {},
        Err(e)=> return Err((client.without_auth(), e.into())),
    }
    Ok(client)
}
