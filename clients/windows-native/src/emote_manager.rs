use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use sha2::{Sha256, Digest};
use anyhow::{Result, Context};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Emote {
    pub name: String,
    pub hash: String,
    pub extension: String,
}

#[derive(Clone)]
pub struct EmoteManager {
    /// User's personal library (name -> Emote)
    pub library: Arc<RwLock<HashMap<String, Emote>>>,
    /// Cache index (hash -> full path)
    pub cache: Arc<RwLock<HashMap<String, PathBuf>>>,
    base_path: PathBuf,
}

impl EmoteManager {
    pub fn new() -> Self {
        let base = get_data_dir();
        let manager = Self {
            library: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
            base_path: base.clone(),
        };
        
        // Ensure directories exist
        let _ = fs::create_dir_all(base.join("library"));
        let _ = fs::create_dir_all(base.join("cache"));
        
        manager.load_library();
        manager.scan_cache();
        
        manager
    }

    fn get_library_json_path(&self) -> PathBuf {
        self.base_path.join("emotes.json")
    }

    fn load_library(&self) {
        let path = self.get_library_json_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(lib) = serde_json::from_str::<HashMap<String, Emote>>(&content) {
                    if let Ok(mut guard) = self.library.write() {
                        *guard = lib;
                    }
                }
            }
        }
    }

    pub fn save_library(&self) -> Result<()> {
        let guard = self.library.read().map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
        let json = serde_json::to_string_pretty(&*guard)?;
        fs::write(self.get_library_json_path(), json)?;
        Ok(())
    }

    fn scan_cache(&self) {
        // Simple scan of cache dir to populate hash map
        // We assume filename is {hash}.{ext}
        let cache_dir = self.base_path.join("cache");
        if let Ok(entries) = fs::read_dir(cache_dir) {
            if let Ok(mut guard) = self.cache.write() {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        // Validate stem is hex hash? For now just assume
                        guard.insert(stem.to_string(), path);
                    }
                }
            }
        }
    }

    /// Import a file into the local library
    pub fn import_emote(&self, source_path: &Path, name: String) -> Result<Emote> {
        let bytes = fs::read(source_path)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = format!("{:x}", hasher.finalize());
        
        let ext = source_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase();
            
        let dest_filename = format!("{}.{}", hash, ext);
        let dest_path = self.base_path.join("library").join(&dest_filename);
        
        fs::copy(source_path, &dest_path)?;
        
        let emote = Emote {
            name: name.clone(),
            hash: hash.clone(),
            extension: ext,
        };
        
        {
            let mut guard = self.library.write().map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
            guard.insert(name, emote.clone());
        }
        
        // Also add to cache index (it points to library file, effectively)
        // Or should cache only track 'cache' dir?
        // Let's simplify: `get_path` checks library dir THEN cache dir. 
        // We don't need to put it in cache map if it's in library map.
        
        self.save_library()?;
        Ok(emote)
    }

    /// Save a received emote data to cache
    pub fn save_to_cache(&self, hash: &str, data_bytes: &[u8]) -> Result<PathBuf> {
        // Detect extension from partial bytes or just assume png/jpg?
        // Usually we might want metadata or detect header.
        // For MVP, lets assume png or detect magic bytes.
        let ext = if data_bytes.starts_with(&[0xFF, 0xD8, 0xFF]) { "jpg" } else { "png" };
        
        let filename = format!("{}.{}", hash, ext);
        let path = self.base_path.join("cache").join(filename);
        
        fs::write(&path, data_bytes)?;
        
        {
            let mut guard = self.cache.write().map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
            guard.insert(hash.to_string(), path.clone());
        }
        
        Ok(path)
    }

    pub fn get_emote_path(&self, hash: &str) -> Option<PathBuf> {
        // 1. Check Library (iterate finding match by hash)
        // This is slow O(N), but library is small.
        {
            let guard = self.library.read().ok()?;
            for emote in guard.values() {
                if emote.hash == hash {
                     let path = self.base_path.join("library").join(format!("{}.{}", emote.hash, emote.extension));
                     if path.exists() { return Some(path); }
                }
            }
        }
        
        // 2. Check Cache
        {
            let guard = self.cache.read().ok()?;
            if let Some(path) = guard.get(hash) {
                if path.exists() { return Some(path.clone()); }
            }
        }
        
        None
    }
    
    pub fn get_emote_by_name(&self, name: &str) -> Option<Emote> {
        let guard = self.library.read().ok()?;
        guard.get(name).cloned()
    }
}

fn get_data_dir() -> PathBuf {
    let base = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    let instance_suffix = std::env::args()
        .skip_while(|a| a != "--instance")
        .nth(1)
        .map(|i| format!("_{}", i))
        .unwrap_or_default();
    
    PathBuf::from(format!("{}/.cryptochat{}/emotes", base, instance_suffix))
}
