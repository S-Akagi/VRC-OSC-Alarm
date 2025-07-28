use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::time::{sleep, Duration};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentTrack {
    pub name: String,
    pub artist: String,
    pub album_art_url: String,
    pub is_playing: bool,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    scope: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CurrentlyPlayingResponse {
    item: Option<Track>,
    is_playing: bool,
}

#[derive(Debug, Deserialize)]
struct Track {
    name: String,
    artists: Vec<Artist>,
    album: Album,
}

#[derive(Debug, Deserialize)]
struct Artist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Album {
    images: Vec<AlbumImage>,
}

#[derive(Debug, Deserialize)]
struct AlbumImage {
    url: String,
    height: u32,
    width: u32,
}

#[derive(Debug)]
pub struct SpotifyManager {
    client: Client,
    client_id: String,
    redirect_uri: String,
    tokens: Arc<Mutex<Option<SpotifyTokens>>>,
}

impl SpotifyManager {
    pub fn new(client_id: String) -> Self {
        Self {
            client: Client::new(),
            client_id,
            redirect_uri: "http://localhost:8888/callback".to_string(),
            tokens: Arc::new(Mutex::new(None)),
        }
    }

    pub fn generate_auth_url(&self) -> Result<(String, String), Box<dyn std::error::Error>> {
        let code_verifier = self.generate_code_verifier();
        let code_challenge = self.generate_code_challenge(&code_verifier)?;
        
        let mut auth_url = Url::parse("https://accounts.spotify.com/authorize")?;
        auth_url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &code_challenge)
            .append_pair("scope", "user-read-currently-playing user-read-playback-state user-modify-playback-state");

        Ok((auth_url.to_string(), code_verifier))
    }

    fn generate_code_verifier(&self) -> String {
        let mut rng = rand::thread_rng();
        let code_verifier: String = (0..128)
            .map(|_| {
                let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
                chars[rng.gen_range(0..chars.len())] as char
            })
            .collect();
        code_verifier
    }

    fn generate_code_challenge(&self, verifier: &str) -> Result<String, Box<dyn std::error::Error>> {
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let result = hasher.finalize();
        Ok(URL_SAFE_NO_PAD.encode(&result))
    }

    pub async fn exchange_code_for_tokens(&self, authorization_code: &str, code_verifier: &str) -> Result<(), Box<dyn std::error::Error>> {
        let params = [
            ("client_id", self.client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", authorization_code),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("code_verifier", code_verifier),
        ];

        let response = self.client
            .post("https://accounts.spotify.com/api/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;

        let token_response: TokenResponse = response.json().await?;
        
        let expires_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + token_response.expires_in;
        
        let tokens = SpotifyTokens {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token.unwrap_or_default(),
            expires_at,
        };

        *self.tokens.lock().unwrap() = Some(tokens);
        Ok(())
    }

    pub async fn refresh_access_token(&self) -> Result<(), Box<dyn std::error::Error>> {
        let refresh_token = {
            let tokens_guard = self.tokens.lock().unwrap();
            match tokens_guard.as_ref() {
                Some(tokens) => tokens.refresh_token.clone(),
                None => return Err("No refresh token available".into()),
            }
        };

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token.as_str()),
            ("client_id", self.client_id.as_str()),
        ];

        let response = self.client
            .post("https://accounts.spotify.com/api/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;

        let token_response: TokenResponse = response.json().await?;
        
        let expires_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + token_response.expires_in;
        
        let new_tokens = SpotifyTokens {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token.unwrap_or(refresh_token),
            expires_at,
        };

        *self.tokens.lock().unwrap() = Some(new_tokens);
        Ok(())
    }

    pub async fn get_current_track(&self) -> Result<Option<CurrentTrack>, Box<dyn std::error::Error>> {
        let access_token = {
            let tokens_guard = self.tokens.lock().unwrap();
            match tokens_guard.as_ref() {
                Some(tokens) => {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                    if now >= tokens.expires_at {
                        drop(tokens_guard);
                        self.refresh_access_token().await?;
                        let refreshed_tokens = self.tokens.lock().unwrap();
                        refreshed_tokens.as_ref().unwrap().access_token.clone()
                    } else {
                        tokens.access_token.clone()
                    }
                }
                None => return Err("No access token available".into()),
            }
        };

        let response = self.client
            .get("https://api.spotify.com/v1/me/player/currently-playing")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        if response.status() == 204 {
            return Ok(None);
        }

        let currently_playing: CurrentlyPlayingResponse = response.json().await?;
        
        if let Some(track) = currently_playing.item {
            let artist_names: Vec<String> = track.artists.iter().map(|a| a.name.clone()).collect();
            let album_art_url = track.album.images.first()
                .map(|img| img.url.clone())
                .unwrap_or_default();

            Ok(Some(CurrentTrack {
                name: track.name,
                artist: artist_names.join(", "),
                album_art_url,
                is_playing: currently_playing.is_playing,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn play_pause(&self) -> Result<(), Box<dyn std::error::Error>> {
        let access_token = self.get_valid_access_token().await?;
        
        let current_track = self.get_current_track().await?;
        let endpoint = if current_track.as_ref().map_or(false, |t| t.is_playing) {
            "https://api.spotify.com/v1/me/player/pause"
        } else {
            "https://api.spotify.com/v1/me/player/play"
        };

        self.client
            .put(endpoint)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        Ok(())
    }

    pub async fn next_track(&self) -> Result<(), Box<dyn std::error::Error>> {
        let access_token = self.get_valid_access_token().await?;
        
        self.client
            .post("https://api.spotify.com/v1/me/player/next")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        Ok(())
    }

    pub async fn previous_track(&self) -> Result<(), Box<dyn std::error::Error>> {
        let access_token = self.get_valid_access_token().await?;
        
        self.client
            .post("https://api.spotify.com/v1/me/player/previous")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        Ok(())
    }

    async fn get_valid_access_token(&self) -> Result<String, Box<dyn std::error::Error>> {
        let tokens_guard = self.tokens.lock().unwrap();
        match tokens_guard.as_ref() {
            Some(tokens) => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                if now >= tokens.expires_at {
                    drop(tokens_guard);
                    self.refresh_access_token().await?;
                    let refreshed_tokens = self.tokens.lock().unwrap();
                    Ok(refreshed_tokens.as_ref().unwrap().access_token.clone())
                } else {
                    Ok(tokens.access_token.clone())
                }
            }
            None => Err("No access token available".into()),
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.tokens.lock().unwrap().is_some()
    }

    pub fn get_tokens(&self) -> Option<SpotifyTokens> {
        self.tokens.lock().unwrap().clone()
    }

    pub fn set_tokens(&self, tokens: SpotifyTokens) {
        *self.tokens.lock().unwrap() = Some(tokens);
    }
}