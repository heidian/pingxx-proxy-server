use serde::Deserialize;

#[derive(Debug)]
pub enum AlipayApiType {
    MAPI,
    OPENAPI,
}

impl<'de> Deserialize<'de> for AlipayApiType {
    fn deserialize<D>(deserializer: D) -> Result<AlipayApiType, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = i32::deserialize(deserializer)?;
        match s {
            1 => Ok(AlipayApiType::MAPI),
            2 => Ok(AlipayApiType::OPENAPI),
            _ => Err(serde::de::Error::custom(format!(
                "unknown alipay_api_type: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
pub enum AlipaySignType {
    #[serde(rename = "rsa")]
    RSA,
    #[serde(rename = "rsa2")]
    RSA256,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AlipayPcDirectConfig {
    pub alipay_pid: String,
    pub alipay_security_key: String,
    pub alipay_account: String,

    pub alipay_version: AlipayApiType,
    pub alipay_app_id: String,

    pub alipay_sign_type: AlipaySignType,
    pub alipay_private_key: String,
    pub alipay_public_key: String,
    pub alipay_private_key_rsa2: String,
    pub alipay_public_key_rsa2: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AlipayWapConfig {
    pub alipay_pid: String,
    pub alipay_security_key: String,
    pub alipay_account: String,

    pub alipay_version: AlipayApiType,
    pub alipay_app_id: String,

    pub alipay_sign_type: AlipaySignType,
    pub alipay_mer_wap_private_key: String,
    pub alipay_wap_public_key: String,
    pub alipay_mer_wap_private_key_rsa2: String,
    pub alipay_wap_public_key_rsa2: String,
}
