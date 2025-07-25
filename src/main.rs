

use grammers_client::session::Session;
use grammers_client::{Client, Config, SignInError};
use simple_logger::SimpleLogger;
use std::io::{self, BufRead as _, Write as _, Result as Res};
use tokio::runtime;
use grammers_client::grammers_tl_types::enums::payments::UniqueStarGift;
use grammers_client::grammers_tl_types as tl;
use std::fs::{self, File};
use std::fs::remove_file;
use std::path::Path;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const SESSION_FILE: &str = "parser.session";

fn prompt(message: &str) -> Result<String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(message.as_bytes())?;
    stdout.flush()?;

    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    let mut line = String::new();
    stdin.read_line(&mut line)?;
    Ok(line)
}

async fn async_main() -> Result<()> {

    let api_id = 27221966;
    let api_hash = "7a547b8a6425910bc9181ecde48e1bcc".to_string();

    println!("Connecting to Telegram...");
    let client = Client::connect(Config {
        session: Session::load_file_or_create(SESSION_FILE)?,
        api_id,
        api_hash: api_hash.clone(),
        params: Default::default(),
    })
    .await?;
    println!("Connected!");

    //  Если есть уже сессия - входим.
    let mut sign_out = false;

    if !client.is_authorized().await? {
        println!("Signing in...");
        let phone = prompt("Enter your phone number (international format): ")?;
        let token = client.request_login_code(&phone).await?;
        let code = prompt("Enter the code you received: ")?;
        let signed_in = client.sign_in(&token, &code).await;
        match signed_in {
            Err(SignInError::PasswordRequired(password_token)) => {
                // Просии ввести номер телефона, код , пароль.
                let hint = password_token.hint().unwrap_or("None");
                let prompt_message = format!("Enter the password (hint {}): ", &hint);
                let password = prompt(prompt_message.as_str())?;

                client
                    .check_password(password_token, password.trim())
                    .await?;
            }
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        };
        println!("Signed in!"); // Вход
        match client.session().save_to_file(SESSION_FILE) {
            Ok(_) => {}
            Err(e) => {
                println!("NOTE: failed to save the session, will sign out when done: {e}");
                sign_out = true;
            }
        }
    }
    let mut gifts = Vec::new();
    let mut gift = prompt("Выберите Slug подарка для парсинга в формате «PlushPepe» ---> ")?;
    let gift = gift.trim();
    let mut i = 1;
    loop {
        let slug = format!("{}-{}", gift, i);
        let get_gift = client.get_unique_star_gift(slug.clone())
        .await;
        match get_gift {
            Ok(UniqueStarGift::Gift(gift)) => {
                println!("Парсинг подарка с номером {}", i);
                gifts.push(UniqueStarGift::Gift(gift));
                i += 1;
            },
            Ok(_) => {
                println!("{}", slug);
                break;
            },
            Err(e) => {
                println!("{}", slug);
                break;
            }
    }
        
        
    }

    if !gifts.is_empty() {
        gen_html(gifts)?;
        println!("Сгенерирован файл с результатом парсинга parsed.html")
    }
    else {
        println!("Не найдено подарков")
    }
    if sign_out {
        // TODO revisit examples and get rid of "handle references" (also, this panics)
        drop(client.sign_out_disconnect().await);
    }

    Ok(())
}

// Функция для генерации удобного и красивого HTML шаблона
// Шаблон сделан с помощью ChatGPT - автор не умеет.
fn gen_html(gifts: Vec<UniqueStarGift>) -> Res<()> {
    let mut html = "<!DOCTYPE html>
<html lang=\"ru\">
<head>
<meta charset=\"UTF-8\" />
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
<title>Telegram Gifts</title>
<style>
  body {
    font-family: \"Segoe UI\", Tahoma, Geneva, Verdana, sans-serif;
    background: #f9fafb;
    color: #2c3e50;
    margin: 0;
    padding: 20px;
  }
  .gifts-container {
    max-width: 900px;
    margin: 0 auto;
  }
  .gift-item {
    background: white;
    border-radius: 8px;
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.1);
    padding: 15px 20px;
    margin-bottom: 15px;
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: center;
  }
  .gift-item a {
    color: #2980b9;
    text-decoration: none;
    font-weight: 600;
  }
  .gift-item a:hover {
    text-decoration: underline;
  }
  .gift-model, .gift-backdrop {
    background: #ecf0f1;
    border-radius: 5px;
    padding: 8px 12px;
    font-size: 14px;
    color: #34495e;
    flex: 1 1 200px;
  }
  .gift-username, .gift-name {
    flex: 0 0 auto;
  }
</style>
</head>
<body>

<div class=\"gifts-container\">
  <!-- Один подарок -->

".to_string();
    let _gifts_info= "Test".to_string();
    for gift in gifts {
        let mut gift_slug: Option<String> = Some(".".to_string());
        let mut _gift_link: Option<String> = Some("зн".to_string());
        let mut gift_model: String = "Test".to_string();
        let mut gift_backdrop: String = "Test".to_string();
        match gift {
            UniqueStarGift::Gift(gift_obj) => {
                match gift_obj.gift {
                    tl::enums::StarGift::Unique(info) => {
                        gift_slug = Some(info.slug.clone());
                        _gift_link = Some(format!("https://t.me/nft/{}", info.slug.clone()));
                        let atr = info.attributes;
                        for elem in atr {
                            match elem {
                                tl::enums::StarGiftAttribute::Backdrop(backdrop) => {
                                    gift_backdrop = backdrop.name;
                                },
                                tl::enums::StarGiftAttribute::Model(model) => {
                                    gift_model = model.name;
                                }
                                _ => {}
                            }
                        }
    
                    },

                    _ => {}
                }
                //match atr  {
                    //tl::enums::StarGiftAttribute::Backdrop(backdrop) => {
                       // gift_backdrop = backdrop.name.clone()
                   // },
                   // _ => {}
                    
             //   }
            },
            _ => {}
            }

        //_gifts_info += _gift_info;
        html.push_str(&format!(
            r#"<div class="gift-item">
    <div class="gift-model">Модель: {}</div>
    <div class="gift-backdrop">Фон: {}</div>
    <a href="{}" class="gift-name" target="_blank" rel="noopener noreferrer">{}</a>
</div>
"#,
            gift_model, gift_backdrop, _gift_link.as_deref().unwrap_or("значение по умолчанию"), gift_slug.as_deref().unwrap_or("значение по умолчанию")
        ));
    }
    html.push_str("</div>\n</body>\n</html>");
    if Path::new("parsed.html").exists() {
        fs::remove_file("parsed.html")?;
    }
    let mut file = File::create("parsed.html")?;
    file.write_all(html.as_bytes())?;
    Ok(())
    
}
fn main() -> Result<()> {
    
    runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main())
}
