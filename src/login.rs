use std::pin::pin;

use color_eyre::eyre::{Context, OptionExt, Report, Result};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures_util::StreamExt;
use jellyfin::JellyfinClient;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, BorderType, Padding, Paragraph, Wrap},
    DefaultTerminal,
};
use serde::{Deserialize, Serialize};

use crate::Config;

#[derive(Debug, Deserialize, Serialize)]
struct LoginInfo {
    server_url: String,
    username: String,
    password: String,
}

enum LoginSelection {
    Server,
    Username,
    Password,
    Retry,
}

async fn get_login_info(
    term: &mut DefaultTerminal,
    info: &mut LoginInfo,
    changed: &mut bool,
    error: Report,
    events: &mut EventStream,
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
    let outer_block = Block::bordered().border_type(BorderType::Rounded).padding(Padding::uniform(4)).title("Enter Jellyfin Server / Login Information");
    loop {
        term.draw(|frame| {
            let server = Paragraph::new(info.server_url.as_str()).block(
                if let LoginSelection::Server = selection {
                    current_block.clone()
                } else {
                    normal_block.clone()
                }.title("Jellyfin URL"),
            );
            let username = Paragraph::new(info.username.as_str()).block(
                if let LoginSelection::Username = selection {
                    current_block.clone()
                } else {
                    normal_block.clone()
                }.title("Username"),
            );
            let password = Paragraph::new(
                Text::from("<hidden>").style(Style::default().add_modifier(Modifier::HIDDEN)),
            )
            .block(if let LoginSelection::Password = selection {
                current_block.clone()
            } else {
                normal_block.clone()
            }.title("Password"));

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
            .areas(outer_block.inner(frame.area()));
            frame.render_widget(&outer_block, frame.area());
            frame.render_widget(server, layout_s);
            frame.render_widget(username, layout_u);
            frame.render_widget(password, layout_p);
            frame.render_widget(button, layout_b);
            frame.render_widget(&error, layout_e);
        })?;
        match events.next().await {
            Some(Ok(Event::Key(KeyEvent {
                code,
                modifiers: _,
                kind: KeyEventKind::Press,
                state: _,
            }))) => match code {
                KeyCode::Backspace => match selection {
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
                KeyCode::Enter => break Ok(true),
                KeyCode::Up => {
                    selection = match selection {
                        LoginSelection::Server => LoginSelection::Retry,
                        LoginSelection::Username => LoginSelection::Server,
                        LoginSelection::Password => LoginSelection::Username,
                        LoginSelection::Retry => LoginSelection::Password,
                    }
                }
                KeyCode::Down | KeyCode::Tab => {
                    selection = match selection {
                        LoginSelection::Server => LoginSelection::Username,
                        LoginSelection::Username => LoginSelection::Password,
                        LoginSelection::Password => LoginSelection::Retry,
                        LoginSelection::Retry => LoginSelection::Server,
                    }
                }
                KeyCode::Char(v) => match selection {
                    LoginSelection::Server => {
                        info.server_url.push(v);
                        *changed = true;
                    }
                    LoginSelection::Username => {
                        info.username.push(v);
                        *changed = true;
                    }
                    LoginSelection::Password => {
                        info.password.push(v);
                        *changed = true;
                    }
                    LoginSelection::Retry => {}
                },
                KeyCode::Esc => break Ok(false),
                _ => {}
            },
            Some(Ok(Event::Paste(v))) => match selection {
                LoginSelection::Server => {
                    info.server_url += &v;
                    *changed = true;
                }
                LoginSelection::Username => {
                    info.username += &v;
                    *changed = true;
                }
                LoginSelection::Password => {
                    info.password += &v;
                    *changed = true;
                }
                LoginSelection::Retry => {}
            },
            Some(Ok(_)) => {}
            Some(Err(e)) => break Err(e).context("receiving terminal events"),
            None => break Ok(false),
        }
    }
}

pub async fn login(
    term: &mut DefaultTerminal,
    config: &Config,
    events: &mut EventStream,
) -> Result<Option<JellyfinClient>> {
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
            };
            error = Some(e);
        }
    }
    let mut info_chainged = false;
    let client = loop {
        if let Some(e) = error.take() {
            if !get_login_info(term, &mut login_info, &mut info_chainged, e, events).await.context("getting login information")?{
                return Ok(None)
            }
        }
        term.draw(|frame| frame.render_widget(&connect_msg, frame.area()))
            .context("rendering ui")?;

        let mut auth_request = pin!(JellyfinClient::new_auth_name(
            &login_info.server_url,
            &login_info.username,
            &login_info.password,
        ));
        tokio::select! {
            event = events.next() => {
                match event {
                    Some(Ok(Event::Key(KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: _,
                        kind: KeyEventKind::Press,
                        state: _,
                    })))
                        | None => return Ok(None),
                    Some(Ok(_)) => {
                        term.draw(|frame| frame.render_widget(&connect_msg, frame.area()))
                            .context("rendering ui")?;
                    }
                    Some(Err(e)) => return Err(e).context("Error getting key events from terminal"),
                }
            }
            request = &mut auth_request => {
                match request.context("logging in") {
                    Ok(client) => break client,
                    Err(e) => error = Some(e),
                }
            }
        };
    };
    if info_chainged {
        std::fs::create_dir_all(&config.login_file.parent().ok_or_eyre("login info path has no parent")?).context("creating login info parent dir")?;
        std::fs::write(
            &config.login_file,
            toml::to_string_pretty(&login_info).context("serializing login info")?,
        )
        .context("writing out new login info")?;
    }
    Ok(Some(client))
}
