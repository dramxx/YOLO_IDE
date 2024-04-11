use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::widget::{
    button, 
    container, 
    text_editor, 
    text, 
    column, 
    row, 
    horizontal_space, 
    tooltip,
    pick_list
};
use iced::{Command, Application, Subscription, Element, Settings, Theme, Font};
use iced::theme;
use iced::executor;
use iced::keyboard;
use iced::highlighter::{self, Highlighter};

fn main() -> iced::Result {
    Editor::run(Settings {
        default_font: Font::MONOSPACE,
        fonts: vec![include_bytes!("../fonts/controls-icons.ttf")
            .as_slice()
            .into()
        ],
        ..Settings::default()
    })
}

struct Editor {
    path: Option<PathBuf>,
    content: text_editor::Content,
    theme: highlighter::Theme,
    is_dirty: bool,
    error: Option<Error>
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    New,
    Open,
    Save,
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    FileSaved(Result<PathBuf, Error>),
    ThemeChanged(highlighter::Theme)
}

#[derive(Debug, Clone)]
enum Error {
    DialogClosed,
    IOFailed(io::ErrorKind)
}

impl Application for Editor {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        (
            Self {
                path: None,
                content: text_editor::Content::new(),
                theme: highlighter::Theme::SolarizedDark,
                is_dirty: true,
                error: None
            },
            Command::perform(
                load_file(default_file()), 
                Message::FileOpened
            ),
        )   
    }

    fn title(&self) -> String {
        String::from("YOLO::IDE")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Edit(action) => {
                self.is_dirty = self.is_dirty || action.is_edit();
                self.content.perform(action);
                self.error = None;

                Command::none()
            }

            Message::New => {
                self.path = None;
                self.content = text_editor::Content::new();
                self.is_dirty = true;
                
                Command::none()
            }

            Message::Open => {
                Command::perform(pick_file(), Message::FileOpened)
            }

            Message::FileOpened(Ok((path, content))) => {
                self.path = Some(path);
                self.content = text_editor::Content::with_text(&content);
                self.is_dirty = false;

                Command::none()
            }
            
            Message::FileOpened(Err(error)) => {
                self.error = Some(error);

                Command::none()
            }

            Message::Save => {
                let text = self.content.text();

                Command::perform(save_file(self.path.clone(), text), Message::FileSaved)
            }

            Message::FileSaved(Ok(path)) => {
                self.path = Some(path);
                self.is_dirty = false;

                Command::none()
            }

            Message::FileSaved(Err(error)) => {
                self.error = Some(error);

                Command::none()
            }

            Message::ThemeChanged(theme) => {
                self.theme = theme;

                Command::none()
            }

       }
    }

    fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key_code, modifiers| {
            match key_code {
                keyboard::Key::Character(_s) if modifiers.command() => Some(Message::Save),
                _ => None
            }
        })
    }

    fn view(&self) -> Element<'_, Message> {
        let controls = row![
            action(new_icon(), "New file", Some(Message::New)),
            action(open_icon(), "Open file", Some(Message::Open)),
            action(save_icon(), "Save file", self.is_dirty.then_some(Message::Save)),
            horizontal_space(),
            pick_list(highlighter::Theme::ALL, Some(self.theme), Message::ThemeChanged)
        ].spacing(10);

        let input = text_editor(&self.content)
            .on_action(Message::Edit)
            .highlight::<Highlighter>(
                highlighter::Settings {
                    theme: self.theme,
                    extension: self
                        .path
                        .as_ref()
                        .and_then(|path| path.extension()?.to_str())
                        .unwrap_or("txt")
                        .to_string()
                },
                |highlight, _theme| highlight.to_format()
            );


        let status_bar = {
            let status = if let Some(Error::IOFailed(error)) = self.error.as_ref() {
                text(error.to_string())
            } else {
                match self.path.as_deref().and_then(Path::to_str) {
                    Some(path) => text(path).size(14),
                    None => text("New file")
                }
            };

            let position = {
                let (line, column) = self.content.cursor_position();
                text(format!("{}:{}", line + 1, column + 1))
            };

            row![
                status,
                horizontal_space(), 
                position
            ]
        };
        
        container(column![
            controls,
            input, 
            status_bar
        ].spacing(10)).padding(10).into()
    }

    fn theme(&self) -> Theme {
        if self.theme.is_dark() {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

fn action<'a>(content: Element<'a, Message>, label: &'a str, message: Option<Message>) -> Element<'a, Message> {
    let is_disabled = message.is_none();
    
    tooltip(
        button(container(content)
            .width(30)
            .center_x())
            .on_press_maybe(message)
            .padding([5, 10])
            .style(
                if is_disabled {
                    theme::Button::Secondary
                } else {
                    theme::Button::Primary
                }
            ),
        label,
        tooltip::Position::FollowCursor
    ).style(theme::Container::Box).into()
}

fn new_icon<'a>() -> Element<'static, Message> {
    icon('\u{F0F6}')
}

fn open_icon<'a>() -> Element<'static, Message> {
    icon('\u{F115}')
}

fn save_icon<'a>() -> Element<'static, Message> {
    icon('\u{E800}')
}

fn icon<'a, Message>(codepoint: char) -> Element<'a, Message> {
    const ICON_FONT: Font = Font::with_name("controls-icons");

    text(codepoint).font(ICON_FONT).into()
}


fn default_file() -> PathBuf {
    PathBuf::from(
        format!(
            "{}/src/main.rs", env!("CARGO_MANIFEST_DIR"))
    )
}

async fn pick_file() -> Result<(PathBuf, Arc<String>), Error>{
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Choose a text file")
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    load_file(handle.path().to_owned()).await
}

async fn load_file(path: PathBuf) -> Result<(PathBuf, Arc<String>), Error> {
    let contents = tokio::fs::read_to_string(&path)
        .await
        .map(Arc::new)
        .map_err(|error| error.kind())
        .map_err(Error::IOFailed)?;

    Ok((path, contents))
}

async fn save_file(path: Option<PathBuf>, text: String) -> Result<PathBuf, Error> {
    let path = if let Some(path) = path { path } else {
        rfd::AsyncFileDialog::new()
            .set_title("Choose a file name")
            .save_file()
            .await
            .ok_or(Error::DialogClosed)
            .map(|handle| handle.path().to_owned())?
    }; 

    tokio::fs::write(&path, text)
        .await
        .map_err(|error| Error::IOFailed(error.kind()))?;

    Ok(path)
}
