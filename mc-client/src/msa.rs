//! Conta do jogador: login Microsoft pelo código de dispositivo (Microsoft → Xbox Live → XSTS → Minecraft) ou
//! um nick offline. A conta fica em `conta.json` na pasta de configuração e o token é renovado sozinho.
//!
//! O app Microsoft (Azure) do projeto vem embutido (`CLIENT_ID`): o jogador só abre o link e digita o código.
//! `MINE_RS_MS_CLIENT_ID` ou o arquivo `microsoft-client-id` na pasta de configuração trocam por outro app.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::auth::Session;

const MS: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/";
const SCOPE: &str = "XboxLive.signin offline_access";

/// Conta salva.
#[derive(Clone, Debug, PartialEq)]
pub enum Account {
    Offline(String),
    Microsoft { name: String, uuid: String, access_token: String, refresh_token: String, client_id: String, expires_at: u64 },
}

/// O que o login em andamento avisa ao jogo.
pub enum Event {
    /// Abra `url` e digite `code`.
    Code { url: String, code: String },
    Done(Result<Account, String>),
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn path() -> PathBuf {
    crate::settings::config_dir().join("conta.json")
}

/// O Client ID do app Microsoft do usuário.
/// App "Minecraft 1.8.9 Rust" no Azure (contas pessoais da Microsoft, fluxo de cliente público). Um Client ID
/// não é segredo: identifica o app na tela de login da Microsoft.
pub const CLIENT_ID: &str = "09922b5b-7377-4cf2-9d16-aa12862adb07";

pub fn client_id() -> Option<String> {
    let custom = std::env::var("MINE_RS_MS_CLIENT_ID").ok().or_else(|| std::fs::read_to_string(crate::settings::config_dir().join("microsoft-client-id")).ok());
    let id = custom.map(|id| id.trim().to_string()).filter(|id| !id.is_empty()).unwrap_or_else(|| CLIENT_ID.to_string());
    Some(id)
}

/// Nick do Minecraft: 3 a 16 letras, números ou `_`.
pub fn valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl Account {
    pub fn name(&self) -> &str {
        match self {
            Account::Offline(n) => n,
            Account::Microsoft { name, .. } => name,
        }
    }

    pub fn session(&self) -> Option<Session> {
        match self {
            Account::Offline(_) => None,
            Account::Microsoft { name, uuid, access_token, .. } => Some(Session { name: name.clone(), uuid: uuid.clone(), access_token: access_token.clone() }),
        }
    }

    pub fn load() -> Option<Self> {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path()).ok()?).ok()?;
        let s = |k: &str| v[k].as_str().map(str::to_string);
        match v["tipo"].as_str()? {
            "offline" => s("name").filter(|n| valid_name(n)).map(Account::Offline),
            "microsoft" => Some(Account::Microsoft {
                name: s("name")?,
                uuid: s("uuid")?,
                access_token: s("access_token")?,
                refresh_token: s("refresh_token")?,
                client_id: s("client_id")?,
                expires_at: v["expires_at"].as_u64().unwrap_or(0),
            }),
            _ => None,
        }
    }

    /// Grava `conta.json` só para o dono (tem o token de renovação).
    pub fn save(&self) -> std::io::Result<()> {
        let v = match self {
            Account::Offline(name) => json!({ "tipo": "offline", "name": name }),
            Account::Microsoft { name, uuid, access_token, refresh_token, client_id, expires_at } => json!({
                "tipo": "microsoft", "name": name, "uuid": uuid, "access_token": access_token,
                "refresh_token": refresh_token, "client_id": client_id, "expires_at": expires_at,
            }),
        };
        let p = path();
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&v)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(tmp, p)
    }

    /// A conta Microsoft com o token válido por mais 5 minutos (renova com o refresh token se preciso).
    pub fn fresh(self) -> Result<Self, String> {
        match &self {
            Account::Microsoft { refresh_token, client_id, expires_at, .. } if *expires_at < now() + 300 => {
                let tok = form(&format!("{MS}token"), &[("client_id", client_id), ("grant_type", "refresh_token"), ("refresh_token", refresh_token), ("scope", SCOPE)])?;
                let acc = minecraft(client_id, &tok)?;
                let _ = acc.save();
                Ok(acc)
            }
            _ => Ok(self),
        }
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).build()
}

/// POST de formulário; as respostas de erro (400 do `authorization_pending`) também voltam como JSON.
fn form(url: &str, fields: &[(&str, &str)]) -> Result<Value, String> {
    match agent().post(url).send_form(fields) {
        Ok(r) => r.into_json().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(_, r)) => r.into_json().map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn post_json(url: &str, body: Value) -> Result<Value, String> {
    match agent().post(url).set("Accept", "application/json").send_json(body) {
        Ok(r) => r.into_json().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(code, r)) => {
            let v: Value = r.into_json().unwrap_or_default();
            // XSTS: 2148916233 sem conta Xbox, 2148916238 conta de menor sem família.
            Err(match v["XErr"].as_u64() {
                Some(2148916233) => "Esta conta Microsoft não tem perfil Xbox; entre uma vez em minecraft.net.".to_string(),
                Some(2148916238) => "Conta de menor de idade: precisa estar numa família Microsoft.".to_string(),
                _ => format!("HTTP {code} em {url}"),
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Do token da Microsoft até o perfil do Minecraft.
fn minecraft(client_id: &str, tok: &Value) -> Result<Account, String> {
    let ms = tok["access_token"].as_str().ok_or_else(|| tok["error_description"].as_str().unwrap_or("login Microsoft recusado").to_string())?;
    let refresh = tok["refresh_token"].as_str().unwrap_or_default().to_string();
    let xbl = post_json(
        "https://user.auth.xboxlive.com/user/authenticate",
        json!({ "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms}") }, "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT" }),
    )?;
    let xbl_token = xbl["Token"].as_str().ok_or("Xbox Live sem token")?;
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or("Xbox Live sem userhash")?;
    let xsts = post_json(
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        json!({ "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] }, "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT" }),
    )?;
    let xsts_token = xsts["Token"].as_str().ok_or("XSTS sem token")?;
    let mc = post_json("https://api.minecraftservices.com/authentication/login_with_xbox", json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }))?;
    let access = mc["access_token"].as_str().ok_or("Minecraft recusou o login (o app Microsoft precisa de acesso à API do Minecraft)")?.to_string();
    let expires = mc["expires_in"].as_u64().unwrap_or(86400);
    let profile: Value = match agent().get("https://api.minecraftservices.com/minecraft/profile").set("Authorization", &format!("Bearer {access}")).call() {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(404, _)) => return Err("Esta conta não tem o Minecraft Java.".into()),
        Err(e) => return Err(e.to_string()),
    };
    Ok(Account::Microsoft {
        name: profile["name"].as_str().ok_or("perfil sem nome")?.to_string(),
        uuid: profile["id"].as_str().ok_or("perfil sem id")?.to_string(),
        access_token: access,
        refresh_token: refresh,
        client_id: client_id.to_string(),
        expires_at: now() + expires,
    })
}

/// Login pelo código de dispositivo numa thread: avisa o código e depois o resultado.
pub fn start_device_login(client_id: String) -> mpsc::Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let res = (|| -> Result<Account, String> {
            let flow = form(&format!("{MS}devicecode"), &[("client_id", &client_id), ("scope", SCOPE)])?;
            let device = flow["device_code"].as_str().ok_or_else(|| flow["error_description"].as_str().unwrap_or("o login Microsoft não começou").to_string())?.to_string();
            let url = flow["verification_uri"].as_str().unwrap_or("https://microsoft.com/link").to_string();
            let code = flow["user_code"].as_str().unwrap_or_default().to_string();
            let _ = tx.send(Event::Code { url, code });
            let interval = flow["interval"].as_u64().unwrap_or(5).max(1);
            let deadline = now() + flow["expires_in"].as_u64().unwrap_or(900);
            while now() < deadline {
                std::thread::sleep(Duration::from_secs(interval));
                let tok = form(&format!("{MS}token"), &[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", &client_id), ("device_code", &device)])?;
                match tok["error"].as_str() {
                    Some("authorization_pending") | Some("slow_down") => continue,
                    Some(_) => return Err(tok["error_description"].as_str().unwrap_or("login Microsoft recusado").to_string()),
                    None => {
                        let acc = minecraft(&client_id, &tok)?;
                        acc.save().map_err(|e| e.to_string())?;
                        return Ok(acc);
                    }
                }
            }
            Err("O código expirou; tente de novo.".into())
        })();
        let _ = tx.send(Event::Done(res));
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nicks_validos() {
        assert!(valid_name("Steve_123"));
        assert!(!valid_name("ab"));
        assert!(!valid_name("nome com espaco"));
        assert!(!valid_name("nomecomdezesseteletras"));
    }
}
