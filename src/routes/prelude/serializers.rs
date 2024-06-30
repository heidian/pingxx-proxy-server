use crate::prisma::sub_app::Data as SubAppData;

impl TryFrom<SubAppData> for serde_json::Value {
    type Error = String;
    fn try_from(data: SubAppData) -> Result<Self, Self::Error> {
        let app = *data
            .app
            .ok_or_else(|| "app on sub_app is required".to_string())?;
        let channel_params = data
            .channel_params
            .ok_or_else(|| "channel_params on sub_app is required".to_string())?;
        let available_methods = channel_params
            .iter()
            .map(|channel_param| channel_param.channel.as_str())
            .collect::<Vec<&str>>();
        let json_data = serde_json::json!({
            "id": data.id,
            "object": "sub_app",
            "created": data.created_at.timestamp(),
            "display_name": data.name,
            "parent_app": app.id,
            "available_methods": available_methods,
            "metadata": {},
        });
        Ok(json_data)
    }
}
