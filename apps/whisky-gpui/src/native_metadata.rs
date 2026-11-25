//! Metadata scraping for the native backend: VNDB, Bangumi, Steam, YMGal and
//! NextMoe over HTTP, normalized into one candidate shape. Applying a
//! candidate stores `metadata.json` plus the downloaded cover under the
//! entry's cache directory, which `library.entries` then surfaces.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::native::jobs::JobStore;
use crate::native::store::Store;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent("Maltoria/0.7 (https://github.com/daydalek/Maltoria)")
        .build()
        .into()
}

fn get_json(url: &str) -> Result<Value, String> {
    let mut response = agent()
        .get(url)
        .call()
        .map_err(|error| format!("{error}"))?;
    response
        .body_mut()
        .read_json()
        .map_err(|error| format!("{error}"))
}

fn post_json(url: &str, body: &Value) -> Result<Value, String> {
    let mut response = agent()
        .post(url)
        .header("Content-Type", "application/json")
        .send_json(body)
        .map_err(|error| format!("{error}"))?;
    response
        .body_mut()
        .read_json()
        .map_err(|error| format!("{error}"))
}

fn download(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let mut response = agent().get(url).call().map_err(|error| format!("{error}"))?;
    let mut reader = response.body_mut().as_reader();
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = std::fs::File::create(dest).map_err(|error| error.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|error| error.to_string())?;
    Ok(())
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(key).and_then(Value::as_str) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

/// VNDB Kana API: the richest visual novel source.
fn search_vndb(query: &str) -> Result<Vec<Value>, String> {
    let body = json!({
        "filters": ["search", "=", query],
        "fields": "title, alttitle, titles.lang, titles.title, released, image.url, description, developers.name, developers.original",
        "results": 20,
        "sort": "searchrank",
        "reverse": true
    });
    let reply = post_json("https://api.vndb.org/kana/vn", &body)?;
    let results = reply
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut games = Vec::new();
    for item in results {
        let title = item
            .get("alttitle")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .or_else(|| item.get("title").and_then(Value::as_str));
        let Some(title) = title else { continue };
        let original = item.get("title").and_then(Value::as_str);
        let developer = item
            .get("developers")
            .and_then(Value::as_array)
            .and_then(|list| list.first())
            .and_then(|dev| {
                dev.get("name")
                    .and_then(Value::as_str)
                    .or_else(|| dev.get("original").and_then(Value::as_str))
            });
        let cover = item
            .get("image")
            .and_then(|image| image.get("url"))
            .and_then(Value::as_str);
        let id = item.get("id").and_then(Value::as_str).unwrap_or("");
        games.push(json!({
            "title": title,
            "originalTitle": original,
            "developer": developer,
            "releaseDate": item.get("released").and_then(Value::as_str),
            "summary": item.get("description").and_then(Value::as_str),
            "coverURL": cover,
            "source": "vndb",
            "sourceID": id,
        }));
    }
    Ok(games)
}

/// Bangumi subject search, game category.
fn search_bangumi(query: &str) -> Result<Vec<Value>, String> {
    let body = json!({
        "keyword": query,
        "filter": { "type": [4] },
        "limit": 20,
    });
    let reply = post_json("https://api.bgm.tv/v0/search/subjects", &body)?;
    let results = reply
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut games = Vec::new();
    for item in results {
        let title = item
            .get("name_cn")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .or_else(|| item.get("name").and_then(Value::as_str));
        let Some(title) = title else { continue };
        let cover = item
            .get("images")
            .and_then(|images| images.get("large").or_else(|| images.get("common")))
            .and_then(Value::as_str);
        let id = item.get("id").and_then(Value::as_i64).unwrap_or(0);
        games.push(json!({
            "title": title,
            "originalTitle": item.get("name").and_then(Value::as_str),
            "developer": Value::Null,
            "releaseDate": item.get("date").and_then(Value::as_str),
            "summary": item.get("summary").and_then(Value::as_str),
            "coverURL": cover,
            "source": "bangumi",
            "sourceID": id.to_string(),
        }));
    }
    Ok(games)
}

/// Steam store search + appdetails for the top hits.
fn search_steam(query: &str) -> Result<Vec<Value>, String> {
    let encoded = query
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect::<String>();
    let reply = get_json(&format!(
        "https://store.steampowered.com/api/storesearch/?term={encoded}&l=schinese&cc=cn"
    ))?;
    let items = reply
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut games = Vec::new();
    for item in items.into_iter().take(8) {
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        let appid = item.get("id").and_then(Value::as_i64).unwrap_or(0);
        let mut detail = json!({});
        if appid > 0 {
            if let Ok(payload) = get_json(&format!(
                "https://store.steampowered.com/api/appdetails?appids={appid}&l=schinese"
            )) {
                detail = payload
                    .get(appid.to_string())
                    .and_then(|row| row.get("data"))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
            }
        }
        let cover = detail
            .get("library_assets")
            .and_then(|assets| assets.get("library_600x900"))
            .and_then(Value::as_str)
            .or_else(|| detail.get("header_image").and_then(Value::as_str))
            .map(str::to_owned)
            .or_else(|| {
                (appid > 0).then(|| {
                    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/library_600x900.jpg")
                })
            });
        games.push(json!({
            "title": name,
            "originalTitle": detail.get("name").and_then(Value::as_str),
            "developer": detail
                .get("developers")
                .and_then(Value::as_array)
                .and_then(|list| list.first())
                .and_then(Value::as_str),
            "releaseDate": detail
                .get("release_date")
                .and_then(|date| date.get("date"))
                .and_then(Value::as_str),
            "summary": detail.get("short_description").and_then(Value::as_str),
            "coverURL": cover,
            "source": "steam",
            "sourceID": appid.to_string(),
        }));
    }
    Ok(games)
}

/// YMGal open API search.
fn search_ymgal(query: &str) -> Result<Vec<Value>, String> {
    let body = json!({ "keyword": query });
    let reply = post_json(
        "https://www.ymgal.games/api/open/archive/search-game",
        &body,
    )?;
    let results = reply
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut games = Vec::new();
    for item in results {
        let title = item
            .get("chineseName")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .or_else(|| item.get("name").and_then(Value::as_str));
        let Some(title) = title else { continue };
        games.push(json!({
            "title": title,
            "originalTitle": item.get("name").and_then(Value::as_str),
            "developer": item.get("orgName").and_then(Value::as_str),
            "releaseDate": item.get("releaseDate").and_then(Value::as_str),
            "summary": item.get("introduction").and_then(Value::as_str),
            "coverURL": item.get("mainImg").and_then(Value::as_str),
            "source": "ymgal",
            "sourceID": item.get("id").and_then(Value::as_i64).map(|id| id.to_string()).unwrap_or_default(),
        }));
    }
    Ok(games)
}

/// NextMoe catalog search.
fn search_nextmoe(query: &str) -> Result<Vec<Value>, String> {
    let encoded = urlencoding(query);
    let reply = get_json(&format!(
        "https://api.nextmoe.dev/v2/catalog?keyword={encoded}&limit=20"
    ))?;
    let results = reply
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| reply.get("items").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    let mut games = Vec::new();
    for item in results {
        let Some(title) = string_field(&item, &["title", "name", "chineseTitle"]) else {
            continue;
        };
        games.push(json!({
            "title": title,
            "originalTitle": string_field(&item, &["originalTitle", "originalName"]),
            "developer": string_field(&item, &["developer", "brand", "brandName"]),
            "releaseDate": string_field(&item, &["releaseDate", "date"]),
            "summary": string_field(&item, &["summary", "description", "introduction"]),
            "coverURL": string_field(&item, &["cover", "coverURL", "image"]),
            "source": "nextmoe",
            "sourceID": string_field(&item, &["id"]).unwrap_or_default(),
        }));
    }
    Ok(games)
}

fn urlencoding(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
                vec![c]
            } else {
                c.to_string().chars().collect()
            }
        })
        .collect::<String>()
        .bytes()
        .map(|b| {
            if (b as char).is_ascii_alphanumeric() || matches!(b as char, '-' | '_' | '.' | '~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn search_source(source: &str, query: &str) -> Result<Vec<Value>, String> {
    match source {
        "vndb" => search_vndb(query),
        "bangumi" => search_bangumi(query),
        "steam" => search_steam(query),
        "ymgal" => search_ymgal(query),
        "nextmoe" => search_nextmoe(query),
        _ => Err(format!("unknown source: {source}")),
    }
}

fn run_search(query: &str, sources: &[String]) -> Value {
    let groups = sources
        .iter()
        .map(|source| {
            let (games, error) = match search_source(source, query) {
                Ok(games) => (games, None),
                Err(reason) => (Vec::new(), Some(reason)),
            };
            let mut group = json!({
                "source": source,
                "games": games,
            });
            if let Some(error) = error {
                group["error"] = json!(error);
            }
            group
        })
        .collect::<Vec<_>>();
    json!({ "results": groups })
}

pub(crate) fn search(params: &Value) -> Result<Value, (String, String)> {
    let query = params
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    if query.trim().is_empty() {
        return Err(("invalid_params".into(), "metadata.search needs query".into()));
    }
    let sources: Vec<String> = params
        .get("sources")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .filter(|list: &Vec<String>| !list.is_empty())
        .unwrap_or_else(|| {
            ["vndb", "bangumi", "steam", "ymgal", "nextmoe"]
                .iter()
                .map(|source| source.to_string())
                .collect()
        });
    // Synchronous network fan-out would stall the dispatch thread for
    // seconds; run providers concurrently and join inside one worker call.
    let mut handles = Vec::new();
    for source in sources {
        let query = query.clone();
        handles.push(std::thread::spawn(move || {
            let (games, error) = match search_source(&source, &query) {
                Ok(games) => (games, None),
                Err(reason) => (Vec::new(), Some(reason)),
            };
            (source, games, error)
        }));
    }
    let mut results = Vec::new();
    for handle in handles {
        if let Ok((source, games, error)) = handle.join() {
            let mut group = json!({
                "source": source,
                "games": games,
            });
            if let Some(error) = error {
                group["error"] = json!(error);
            }
            results.push(group);
        }
    }
    Ok(json!({ "results": results }))
}

fn normalize_metadata(candidate: &Value) -> Value {
    let title = string_field(candidate, &["title"]);
    let mut provenance = serde_json::Map::new();
    let source = candidate
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    for key in ["title", "summary", "developer", "releaseDate", "cover"] {
        provenance.insert(
            key.to_owned(),
            json!({ "source": source, "manual": false }),
        );
    }
    json!({
        "title": title,
        "summary": string_field(candidate, &["summary"]),
        "developer": string_field(candidate, &["developer"]),
        "releaseDate": string_field(candidate, &["releaseDate"]),
        "provenance": provenance,
    })
}

pub(crate) fn apply(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let entry_id = params
        .get("entryID")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "metadata.apply needs entryID".into()))?
        .to_owned();
    let candidate = params
        .get("metadata")
        .cloned()
        .ok_or_else(|| ("invalid_params".into(), "metadata.apply needs metadata".into()))?;
    let dir = store.metadata_dir(&entry_id);
    let _ = std::fs::create_dir_all(&dir);
    let metadata = normalize_metadata(&candidate);
    let data = serde_json::to_vec_pretty(&metadata).map_err(|error| {
        ("encode_failed".into(), error.to_string())
    })?;
    std::fs::write(dir.join("metadata.json"), &data)
        .map_err(|error| ("write_failed".into(), error.to_string()))?;
    // Best-effort cover download; failure leaves the placeholder art.
    if let Some(cover) = candidate.get("coverURL").and_then(Value::as_str) {
        let extension = cover
            .rsplit('.')
            .next()
            .filter(|ext| ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
            .unwrap_or("jpg");
        let target = dir.join(format!("cover.{extension}"));
        let _ = download(cover, &target);
    }
    // Record the provider identity so refresh can re-pull the same record.
    store.update_entry(
        &entry_id,
        "metadataSource",
        json!({
            "source": candidate.get("source").and_then(Value::as_str),
            "sourceID": candidate.get("sourceID").and_then(Value::as_str),
        }),
    );
    Ok(json!({ "applied": true }))
}

pub(crate) fn refresh(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let entry_id = params
        .get("entryID")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "metadata.refresh needs entryID".into()))?;
    let record = store.entry_override(entry_id);
    let source = record
        .get("metadataSource")
        .and_then(|row| row.get("source"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ("no_source".into(), "No metadata source recorded for this entry".into())
        })?;
    let title = load_title(store, entry_id).unwrap_or_else(|| entry_id.to_owned());
    let games = search_source(source, &title)
        .map_err(|error| ("search_failed".into(), error))?;
    let Some(candidate) = games.first() else {
        return Err(("no_match".into(), "The source returned no match".into()));
    };
    apply(store, &json!({
        "entryID": entry_id,
        "metadata": candidate,
    }))
}

fn load_title(store: &Store, entry_id: &str) -> Option<String> {
    let data = std::fs::read(store.metadata_dir(entry_id).join("metadata.json")).ok()?;
    let value: Value = serde_json::from_slice(&data).ok()?;
    value
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

pub(crate) fn edit(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let entry_id = params
        .get("entryID")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "metadata.edit needs entryID".into()))?;
    let dir = store.metadata_dir(entry_id);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("metadata.json");
    let mut metadata: Value = std::fs::read(&path)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_else(|| json!({}));
    if let Some(edits) = params.get("fields").or_else(|| params.get("metadata")) {
        for key in ["title", "summary", "developer", "releaseDate"] {
            if let Some(value) = edits.get(key) {
                if value.is_null() {
                    if let Some(map) = metadata.as_object_mut() {
                        map.remove(key);
                    }
                } else {
                    metadata[key] = value.clone();
                }
                // Manual edits own their provenance category.
                let category = if key == "releaseDate" { "releaseDate" } else { key };
                if let Some(provenance) = metadata
                    .as_object_mut()
                    .and_then(|map| map.get_mut("provenance"))
                    .and_then(Value::as_object_mut)
                {
                    provenance.insert(category.to_owned(), json!({ "manual": true }));
                } else {
                    metadata["provenance"] = json!({ category: { "manual": true } });
                }
            }
        }
    }
    let data = serde_json::to_vec_pretty(&metadata)
        .map_err(|error| ("encode_failed".into(), error.to_string()))?;
    std::fs::write(&path, &data).map_err(|error| ("write_failed".into(), error.to_string()))?;
    Ok(json!({ "applied": true }))
}

pub(crate) fn clear(store: &Store, params: &Value) -> Result<Value, (String, String)> {
    let entry_id = params
        .get("entryID")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "metadata.clear needs entryID".into()))?;
    let dir = store.metadata_dir(entry_id);
    let _ = std::fs::remove_dir_all(&dir);
    Ok(json!({ "cleared": true }))
}

// ---------------------------------------------------------------------------
// Record-based configuration model (`metadata.configuration.*`)
// ---------------------------------------------------------------------------

fn fetch_vndb(id: &str, include_characters: bool) -> Result<Value, String> {
    let body = json!({
        "filters": ["id", "=", id],
        "fields": "title, alttitle, released, image.url, description, developers.name",
        "results": 1
    });
    let reply = post_json("https://api.vndb.org/kana/vn", &body)?;
    let item = reply
        .get("results")
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .cloned()
        .ok_or_else(|| format!("vndb: no record for {id}"))?;
    let title = item
        .get("alttitle")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .or_else(|| item.get("title").and_then(Value::as_str))
        .unwrap_or(id);
    let mut snapshot = json!({
        "title": title,
        "originalTitle": item.get("title").and_then(Value::as_str),
        "developer": item.get("developers").and_then(Value::as_array).and_then(|l| l.first()).and_then(|d| d.get("name")).and_then(Value::as_str),
        "releaseDate": item.get("released").and_then(Value::as_str),
        "summary": item.get("description").and_then(Value::as_str),
        "coverURL": item.get("image").and_then(|i| i.get("url")).and_then(Value::as_str),
        "source": "vndb",
        "sourceID": id,
    });
    if include_characters {
        let chars_body = json!({
            "filters": ["vn", "=", ["id", "=", id]],
            "fields": "name, original, image.url, vns.role, vns.id",
            "results": 50
        });
        if let Ok(chars_reply) = post_json("https://api.vndb.org/kana/character", &chars_body) {
            if let Some(list) = chars_reply.get("results").and_then(Value::as_array) {
                let characters: Vec<Value> = list.iter().map(|c| {
                    let role = c.get("vns").and_then(Value::as_array)
                        .and_then(|vns| vns.first())
                        .and_then(|vn| vn.get("role")).and_then(Value::as_str).unwrap_or("");
                    json!({
                        "id": c.get("id").and_then(Value::as_str).unwrap_or(""),
                        "name": c.get("name").and_then(Value::as_str).unwrap_or(""),
                        "originalName": c.get("original").and_then(Value::as_str),
                        "imageURL": c.get("image").and_then(|i| i.get("url")).and_then(Value::as_str),
                        "role": if role == "main" { "main" } else { "side" },
                    })
                }).collect();
                snapshot["characters"] = json!(characters);
            }
        }
    }
    Ok(snapshot)
}

fn fetch_bangumi(id: &str, include_characters: bool) -> Result<Value, String> {
    let item = get_json(&format!("https://api.bgm.tv/v0/subjects/{id}"))?;
    let title = item
        .get("name_cn")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .or_else(|| item.get("name").and_then(Value::as_str))
        .unwrap_or(id);
    let developer = item
        .get("infobox")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.get("key").and_then(Value::as_str) == Some("开发")
            }).cloned()
        })
        .and_then(|row| row.get("value").cloned())
        .and_then(|value| {
            if let Some(text) = value.as_str() {
                return Some(text.to_owned());
            }
            value.as_array().and_then(|list| list.first()).and_then(|v| {
                v.get("v").and_then(Value::as_str).map(str::to_owned)
            })
        });
    let mut snapshot = json!({
        "title": title,
        "originalTitle": item.get("name").and_then(Value::as_str),
        "developer": developer,
        "releaseDate": item.get("date").and_then(Value::as_str),
        "summary": item.get("summary").and_then(Value::as_str),
        "coverURL": item.get("images").and_then(|i| i.get("large")).and_then(Value::as_str),
        "source": "bangumi",
        "sourceID": id,
    });
    if include_characters {
        if let Ok(chars) = get_json(&format!("https://api.bgm.tv/v0/subjects/{id}/characters")) {
            if let Some(list) = chars.as_array() {
                let characters: Vec<Value> = list.iter().map(|c| {
                    json!({
                        "id": c.get("id").and_then(Value::as_i64).map(|i| i.to_string()).unwrap_or_default(),
                        "name": c.get("name").and_then(Value::as_str).unwrap_or(""),
                        "originalName": Value::Null,
                        "imageURL": c.get("images").and_then(|i| i.get("medium").or_else(|| i.get("small"))).and_then(Value::as_str),
                        "role": if c.get("relation").and_then(Value::as_str) == Some("主角") { "main" } else { "side" },
                        "voiceActor": c.get("actors").and_then(Value::as_array).and_then(|a| a.first()).and_then(|a| a.get("name")).and_then(Value::as_str),
                    })
                }).collect();
                snapshot["characters"] = json!(characters);
            }
        }
    }
    Ok(snapshot)
}

fn fetch_steam(id: &str) -> Result<Value, String> {
    let payload = get_json(&format!(
        "https://store.steampowered.com/api/appdetails?appids={id}&l=schinese"
    ))?;
    let detail = payload
        .get(id)
        .and_then(|row| row.get("data"))
        .cloned()
        .ok_or_else(|| format!("steam: no data for {id}"))?;
    let portrait = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/library_600x900.jpg");
    Ok(json!({
        "title": detail.get("name").and_then(Value::as_str).unwrap_or(id),
        "originalTitle": detail.get("name").and_then(Value::as_str),
        "developer": detail.get("developers").and_then(Value::as_array).and_then(|l| l.first()).and_then(Value::as_str),
        "releaseDate": detail.get("release_date").and_then(|d| d.get("date")).and_then(Value::as_str),
        "summary": detail.get("short_description").and_then(Value::as_str),
        "coverURL": portrait,
        "source": "steam",
        "sourceID": id,
    }))
}

fn fetch_ymgal(id: &str) -> Result<Value, String> {
    let reply = get_json(&format!("https://www.ymgal.games/api/open/archive/game?gaId={id}"))?;
    let item = reply.get("data").cloned().unwrap_or(reply);
    Ok(json!({
        "title": string_field(&item, &["chineseName", "name"]).unwrap_or_else(|| id.to_owned()),
        "originalTitle": item.get("name").and_then(Value::as_str),
        "developer": item.get("orgName").and_then(Value::as_str),
        "releaseDate": item.get("releaseDate").and_then(Value::as_str),
        "summary": item.get("introduction").and_then(Value::as_str),
        "coverURL": item.get("mainImg").and_then(Value::as_str),
        "source": "ymgal",
        "sourceID": id,
    }))
}

fn fetch_nextmoe(id: &str) -> Result<Value, String> {
    let reply = get_json(&format!("https://api.nextmoe.dev/v2/catalog/{id}"))?;
    let item = reply.get("data").cloned().unwrap_or(reply);
    Ok(json!({
        "title": string_field(&item, &["title", "name"]).unwrap_or_else(|| id.to_owned()),
        "originalTitle": string_field(&item, &["originalTitle"]),
        "developer": string_field(&item, &["developer", "brand"]),
        "releaseDate": string_field(&item, &["releaseDate"]),
        "summary": string_field(&item, &["summary", "description"]),
        "coverURL": string_field(&item, &["cover", "coverURL", "image"]),
        "source": "nextmoe",
        "sourceID": id,
    }))
}

pub(crate) fn fetch_snapshot(source: &str, id: &str, include_characters: bool) -> Result<Value, String> {
    match source {
        "vndb" => fetch_vndb(id, include_characters),
        "bangumi" => fetch_bangumi(id, include_characters),
        "steam" => fetch_steam(id),
        "ymgal" => fetch_ymgal(id),
        "nextmoe" => fetch_nextmoe(id),
        _ => Err(format!("unknown source: {source}")),
    }
}

fn capabilities() -> Value {
    json!({
        "steam": { "refresh": true, "characters": false },
        "bangumi": { "refresh": true, "characters": true },
        "vndb": { "refresh": true, "characters": true },
        "ymgal": { "refresh": true, "characters": false },
        "nextmoe": { "refresh": true, "characters": false },
    })
}

fn default_record() -> Value {
    json!({
        "version": 1,
        "defaultSource": "steam",
        "categorySources": {},
        "bindings": {},
        "snapshots": {},
    })
}

fn record_path(store: &Store, entry_id: &str) -> std::path::PathBuf {
    store.metadata_dir(entry_id).join("record.json")
}

fn load_record(store: &Store, entry_id: &str) -> Value {
    std::fs::read(record_path(store, entry_id))
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_else(default_record)
}

/// Resolve the record into the flat metadata.json the library surfaces.
fn resolve_record(store: &Store, entry_id: &str, record: &Value) {
    let default_source = record["defaultSource"].as_str().unwrap_or("steam");
    let category_source = |category: &str| -> &str {
        record["categorySources"][category]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(default_source)
    };
    let snapshot = |source: &str| -> Value {
        record["snapshots"][source].clone()
    };
    let pick = |category: &str, keys: &[&str]| -> Value {
        let source = category_source(category);
        for key in keys {
            if let Some(value) = snapshot(source).get(*key).filter(|v| !v.is_null()) {
                return value.clone();
            }
        }
        if let Some(bindings) = record["bindings"].as_object() {
            for source in bindings.keys() {
                for key in keys {
                    if let Some(value) = snapshot(source).get(*key).filter(|v| !v.is_null()) {
                        return value.clone();
                    }
                }
            }
        }
        Value::Null
    };
    let mut provenance = serde_json::Map::new();
    for (category, prov_key) in [("basic", "title"), ("summary", "summary"), ("cover", "cover"), ("characters", "characters")] {
        let source = category_source(category);
        if record["bindings"][source].is_string() {
            provenance.insert(prov_key.into(), json!({ "source": source, "manual": false }));
        }
    }
    for key in ["title", "originalTitle", "developer", "releaseDate", "summary"] {
        if record[key].is_string() {
            provenance.insert(key.into(), json!({ "manual": true }));
        }
    }
    let manual = |key: &str, fallback: Value| -> Value {
        if record[key].is_string() { record[key].clone() } else { fallback }
    };
    let characters = {
        let source = category_source("characters");
        snapshot(source)["characters"].clone()
    };
    let metadata = json!({
        "title": manual("title", pick("basic", &["title"])),
        "summary": manual("summary", pick("summary", &["summary"])),
        "developer": manual("developer", pick("basic", &["developer"])),
        "releaseDate": manual("releaseDate", pick("basic", &["releaseDate"])),
        "characters": if characters.is_array() { characters } else { Value::Null },
        "provenance": provenance,
    });
    let dir = store.metadata_dir(entry_id);
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(data) = serde_json::to_vec_pretty(&metadata) {
        let _ = std::fs::write(dir.join("metadata.json"), data);
    }
    let cover_url = {
        let source = category_source("cover");
        snapshot(source)["coverURL"].as_str().map(str::to_owned).or_else(|| {
            record["bindings"].as_object().and_then(|bindings| {
                bindings.keys().find_map(|s| snapshot(s)["coverURL"].as_str().map(str::to_owned))
            })
        })
    };
    if let Some(url) = cover_url {
        let extension = url.rsplit('.').next()
            .filter(|ext| ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
            .unwrap_or("jpg");
        let target = dir.join(format!("cover.{extension}"));
        let _ = download(&url, &target);
    }
}

pub(crate) fn configuration(store: &Store, method: &str, params: &Value) -> Result<Value, (String, String)> {
    let entry_id = params
        .get("entryID")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), format!("{method} needs entryID")))?;
    let dir = store.metadata_dir(entry_id);
    let mut record = load_record(store, entry_id);
    let mut errors = serde_json::Map::new();
    if method == "metadata.configuration.save" {
        let draft = params.get("record").cloned().unwrap_or_else(|| record.clone());
        record = draft;
        let include_characters = params["includeCharacters"].as_bool().unwrap_or(true);
        let hydrate = params["hydrate"].as_bool().unwrap_or(true);
        if hydrate {
            if let Some(bindings) = record["bindings"].as_object().cloned() {
                for (source, id_value) in bindings {
                    let Some(id) = id_value.as_str() else { continue };
                    let unchanged = record["snapshots"][&source]["sourceID"].as_str() == Some(id)
                        && !record["snapshots"][&source]["characters"].is_null();
                    if unchanged { continue; }
                    match fetch_snapshot(&source, id, include_characters) {
                        Ok(snapshot) => {
                            record["snapshots"][&source] = snapshot;
                        }
                        Err(reason) => {
                            errors.insert(source.clone(), json!(reason));
                        }
                    }
                }
            }
        }
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(data) = serde_json::to_vec_pretty(&record) {
            let _ = std::fs::write(record_path(store, entry_id), data);
        }
        resolve_record(store, entry_id, &record);
    } else if method == "metadata.refresh" {
        let include_characters = params["includeCharacters"].as_bool().unwrap_or(true);
        if let Some(bindings) = record["bindings"].as_object().cloned() {
            for (source, id_value) in bindings {
                let Some(id) = id_value.as_str() else { continue; };
                if let Some(sources) = params.get("sources").and_then(Value::as_array) {
                    if !sources.iter().any(|s| s.as_str() == Some(source.as_str())) {
                        continue;
                    }
                }
                match fetch_snapshot(&source, id, include_characters) {
                    Ok(snapshot) => {
                        record["snapshots"][&source] = snapshot;
                    }
                    Err(reason) => {
                        errors.insert(source.clone(), json!(reason));
                    }
                }
            }
        }
        if let Ok(data) = serde_json::to_vec_pretty(&record) {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(record_path(store, entry_id), data);
        }
        resolve_record(store, entry_id, &record);
    }
    let has_record = record_path(store, entry_id).is_file();
    Ok(json!({
        "record": record,
        "capabilities": capabilities(),
        "hasRecord": has_record,
        "errors": errors,
    }))
}

/// `metadata.cover.import` copies a user-picked image into the cache;
/// `metadata.cover.reset` deletes it.
pub(crate) fn cover(store: &Store, method: &str, params: &Value) -> Result<Value, (String, String)> {
    let entry_id = params
        .get("entryID")
        .or_else(|| params.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), format!("{method} needs entryID")))?;
    let dir = store.metadata_dir(entry_id);
    if method == "metadata.cover.reset" {
        for name in ["cover.jpg", "cover.png", "cover.webp", "cover.jpeg"] {
            let _ = std::fs::remove_file(dir.join(name));
        }
        return Ok(json!({ "reset": true }));
    }
    let source = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "metadata.cover.import needs path".into()))?;
    let extension = std::path::Path::new(source)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("jpg")
        .to_lowercase();
    let _ = std::fs::create_dir_all(&dir);
    let target = dir.join(format!("cover.{extension}"));
    std::fs::copy(source, &target).map_err(|error| ("copy_failed".into(), error.to_string()))?;
    Ok(json!({ "imported": true }))
}
