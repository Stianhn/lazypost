use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct CollectionsResponse {
    pub collections: Vec<CollectionInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    pub uid: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollectionDetailResponse {
    pub collection: CollectionDetail,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollectionDetail {
    pub info: CollectionDetailInfo,
    #[serde(default)]
    pub item: Vec<Item>,
    #[serde(default)]
    pub variable: Vec<Variable>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CollectionDetailInfo {
    #[serde(rename = "_postman_id")]
    pub postman_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Item {
    Request(RequestItem),
    Folder(Folder),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Folder {
    pub name: String,
    #[serde(default)]
    pub item: Vec<Item>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub request: Request,
    #[serde(default)]
    pub response: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Request {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub url: RequestUrl,
    #[serde(default)]
    pub header: Vec<Header>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<RequestBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(untagged)]
pub enum RequestUrl {
    Complex(UrlDetail),
    Simple(String),
    #[default]
    Empty,
}

impl Serialize for RequestUrl {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            RequestUrl::Complex(detail) => detail.serialize(serializer),
            RequestUrl::Simple(s) => serializer.serialize_str(s),
            RequestUrl::Empty => serializer.serialize_str(""),
        }
    }
}

impl RequestUrl {
    pub fn to_string(&self) -> String {
        match self {
            RequestUrl::Simple(s) => s.clone(),
            RequestUrl::Complex(u) => u.raw.clone().unwrap_or_default(),
            RequestUrl::Empty => String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UrlDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query: Vec<QueryParam>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QueryParam {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Header {
    pub key: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_bool_option")]
    pub disabled: Option<bool>,
}

fn deserialize_bool_option<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct BoolOptionVisitor;

    impl<'de> Visitor<'de> for BoolOptionVisitor {
        type Value = Option<bool>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a boolean, a string 'true'/'false', or null")
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
            Ok(Some(v))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match v.to_lowercase().as_str() {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                "" => Ok(None),
                _ => Err(de::Error::custom(format!("invalid boolean string: {}", v))),
            }
        }

        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(BoolOptionVisitor)
        }
    }

    deserializer.deserialize_any(BoolOptionVisitor)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExecutedResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

// Environment models
#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentsResponse {
    pub environments: Vec<EnvironmentInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentInfo {
    pub name: String,
    pub uid: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentDetailResponse {
    pub environment: EnvironmentDetail,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentDetail {
    #[serde(default)]
    pub values: Vec<Variable>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Variable {
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

// Workspace models
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspacesResponse {
    pub workspaces: Vec<WorkspaceInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
}
