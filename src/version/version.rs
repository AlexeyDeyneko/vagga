use std::path::BytesContainer;

use config::builders as B;

use container::sha256::Digest;


pub enum HashResult {
    Hashed,
    New,
    Error(String)
}


pub trait VersionHash {
    fn hash(&self, hash: &mut Digest) -> HashResult;
}


impl VersionHash for B::Builder {
    fn hash(&self, hash: &mut Digest) -> HashResult {
        match self {
            &B::UbuntuCore(ref name) => {
                // TODO(tailhook) get hash of the downloaded image
                debug!("Add to hash `UbuntuCore:{}`", name);
                hash.input("UbuntuCore:".as_bytes());
                hash.input(name.as_bytes());
                hash.input(&[0]);
                Hashed
            }
            &B::Cmd(ref vec) => {
                vec.iter().all(|cmd| {
                    hash.input(cmd.as_bytes());
                    hash.input(&[0]);
                    true
                });
                Hashed
            }
            &B::Sh(ref cmd) => {
                hash.input(cmd.as_bytes());
                hash.input(&[0]);
                Hashed
            }
            &B::Env(ref pairs) => {
                for (k, v) in pairs.iter() {
                    hash.input(k.as_bytes());
                    hash.input(&[0]);
                    hash.input(v.as_bytes());
                    hash.input(&[0]);
                }
                Hashed
            }
            &B::Remove(ref path) | &B::EnsureDir(ref path) |
            &B::EmptyDir(ref path) => {
                hash.input(path.container_as_bytes());
                hash.input(&[0]);
                Hashed
            }
        }
    }
}
