//! Types and helpers for `kubeconfig` parsing.

// Lifted from https://github.com/camallo/k8s-client-rs/blob/master/src/kubeconfig.rs
// until a more complete kubernetes client exists

use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use serde_yaml;
use openssl::x509::X509;
use openssl::pkey::{PKey, Private};
use url::Url;
use url_serde;
use base64;
use errors::*;

/// Configuration to build a Kubernetes client.
#[derive(Debug, Serialize, Deserialize)]
pub struct KubeConfig {
    pub kind: Option<String>,
    #[serde(rename = "apiVersion")]
    pub api_version: Option<String>,
    pub preferences: Option<Preferences>,
    pub clusters: Vec<NamedCluster>,
    pub users: Vec<NamedAuthInfo>,
    pub contexts: Vec<NamedContext>,
    #[serde(rename = "current-context")]
    pub current_context: String,
    pub extensions: Option<Vec<NamedExtension>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Preferences {
    pub colors: Option<bool>,
    pub extensions: Option<Vec<NamedExtension>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NamedCluster {
    pub name: String,
    pub cluster: Cluster,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cluster {
    #[serde(with = "url_serde")]
    pub server: Url,
    #[serde(rename = "insecure-skip-tls-verify")]
    pub insecure_tls: Option<bool>,
    #[serde(rename = "certificate-authority")]
    ca_file: Option<String>,
    #[serde(rename = "certificate-authority-data")]
    ca_data: Option<String>,
    pub extensions: Option<Vec<NamedExtension>>,
}

/// Given two Option<String> parameters representing
/// base64 encoded data, or file name with the data
/// return the data form the first found in the specified order.
fn get_from_b64data_or_file(data: &Option<String>, file: &Option<String>) -> Option<String> {
    if let &Some(ref data) = data {
        let decoded = base64::decode(&data).expect("Unable to decode base64.");
        Some(String::from_utf8(decoded).expect("Unable to convert to string."))
    } else if let &Some(ref file) = file {
        let mut data = String::new();
        let mut f = File::open(file).expect("Unable to open file.");
        f.read_to_string(&mut data)
            .expect("File data is not UTF-8.");
        Some(data)
    } else {
        None
    }
}

impl Cluster {
    pub fn ca_cert(&self) -> Option<X509> {
        get_from_b64data_or_file(&self.ca_data, &self.ca_file).map(|k| {
            X509::from_pem(k.as_ref()).expect("Invalid kubeconfig - ca cert is not PEM-encoded")
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamedAuthInfo {
    pub name: String,
    pub user: AuthInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthProviderConfig {
    #[serde(rename = "access-token")]
    pub access_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthProvider {
    pub config: Option<AuthProviderConfig>,
    pub name: Option<String>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthInfo {
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(rename = "auth-provider")]
    pub auth_provider: Option<AuthProvider>,
    pub token: Option<String>,
    #[serde(rename = "tokenFile")]
    pub token_file: Option<String>,
    #[serde(rename = "client-certificate")]
    client_certificate_file: Option<String>,
    #[serde(rename = "client-certificate-data")]
    client_certificate_data: Option<String>,
    #[serde(rename = "client-key")]
    pub client_key_file: Option<String>,
    #[serde(rename = "client-key-data")]
    pub client_key_data: Option<String>,
    pub impersonate: Option<String>,
    //TODO
}

impl AuthInfo {
    pub fn client_certificate(&self) -> Option<X509> {
        get_from_b64data_or_file(&self.client_certificate_data, &self.client_certificate_file)
            .map(|k| X509::from_pem(k.as_ref())
                .expect("Invalid kubeconfig - client cert is not PEM-encoded"))
    }
    pub fn client_key(&self) -> Option<PKey<Private>> {
        get_from_b64data_or_file(&self.client_key_data, &self.client_key_file)
            .map(|k| PKey::<Private>::private_key_from_pem(k.as_ref())
                .expect("Invalid kubeconfig - client key is not PEM-encoded"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamedContext {
    pub name: String,
    pub context: Context,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Context {
    pub cluster: String,
    pub user: String,
    pub namespace: Option<String>,
    pub extensions: Option<Vec<Extension>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamedExtension {
    pub name: String,
    pub extension: Extension,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Extension {
    pub extension: String,
}

#[derive(Clone, Debug)]
pub struct ClusterContext {
    pub name: String,
    pub cluster: Cluster,
    pub user: AuthInfo,
    pub namespace: Option<String>,
    pub extensions: Option<Vec<Extension>>,
}

impl KubeConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<KubeConfig> {
        let f = File::open(path.as_ref()).chain_err(|| "Unable to open kubeconfig file")?;
        serde_yaml::from_reader(f).chain_err(|| "Unable to parse kubeconfig file")
    }

    pub fn context(&self, name: &str) -> Result<ClusterContext> {
        let ctxs: Vec<&NamedContext> = self.contexts.iter().filter(|c| c.name == name).collect();
        let ctx = match ctxs.len() {
            0 => bail!("unknown context {}", name),
            1 => &ctxs[0].context,
            _ => bail!("ambiguous context {}", name),
        };
        let clus: Vec<&NamedCluster> = self.clusters
            .iter()
            .filter(|c| c.name == ctx.cluster)
            .collect();
        let clu = match clus.len() {
            0 => bail!("unknown cluster {}", name),
            1 => &clus[0].cluster,
            _ => bail!("ambiguous cluster {}", name),
        };
        let auths: Vec<&NamedAuthInfo> = self.users
            .iter()
            .filter(|c| c.name == ctx.user)
            .collect();
        let auth = match auths.len() {
            0 => bail!("unknown auth-info {}", name),
            1 => &auths[0].user,
            _ => bail!("ambiguous auth-info {}", name),
        };
        let rc = ClusterContext {
            name: name.to_string(),
            cluster: clu.clone(),
            user: auth.clone(),
            namespace: ctx.namespace.clone(),
            extensions: None,
        };
        Ok(rc)
    }

    pub fn default_context(&self) -> Result<ClusterContext> {
        let dname = self.current_context.as_ref();
        self.context(dname)
    }

    pub fn default_path() -> PathBuf {
        env::home_dir()
            .unwrap_or("/root".into())
            .join(".kube")
            .join("config")
    }
}
