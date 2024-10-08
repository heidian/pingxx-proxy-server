use rand::Rng;

pub fn api_base() -> String {
    std::env::var("API_BASE").unwrap()
}

pub fn generate_id(prefix: &str) -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let mut rng = rand::thread_rng();
    let number: u64 = rng.gen_range(10000000000..100000000000);
    format!("{}{}{}", prefix, timestamp, number)
}

pub fn charge_notify_url(charge_id: &str) -> String {
    let api_base = std::env::var("API_BASE").unwrap();
    format!("{}/notify/charges/{}", api_base, charge_id)
    // "https://notify.pingxx.com/notify/charges/ch_101240601691280343040013";
}

pub fn refund_notify_url(charge_id: &str, refund_id: &str) -> String {
    let api_base = std::env::var("API_BASE").unwrap();
    format!(
        "{}/notify/charges/{}/refunds/{}",
        api_base, charge_id, refund_id
    )
}

pub fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut index = max_bytes;
    while !s.is_char_boundary(index) {
        index -= 1;
    }
    &s[..index]
}

mod db {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum DBError {
        #[error("[Error executing SQL] {0}")]
        SQLFailed(String),
        #[error("[DoesNotExist] {0}")]
        DoesNotExist(String),
    }

    impl From<prisma_client_rust::QueryError> for DBError {
        fn from(e: prisma_client_rust::QueryError) -> Self {
            DBError::SQLFailed(format!("{:?}", e))
        }
    }

    pub async fn load_order_from_db(
        prisma_client: &crate::prisma::PrismaClient,
        order_id: &str,
    ) -> Result<
        (
            crate::prisma::order::Data,
            Vec<crate::prisma::charge::Data>,
            crate::prisma::app::Data,
            crate::prisma::sub_app::Data,
        ),
        DBError,
    > {
        let mut order = prisma_client
            .order()
            .find_unique(crate::prisma::order::id::equals(order_id.to_string()))
            .with(crate::prisma::order::sub_app::fetch())
            .with(crate::prisma::order::app::fetch())
            .with(
                crate::prisma::order::charges::fetch(vec![]).order_by(
                    crate::prisma::charge::created_at::order(prisma_client_rust::Direction::Desc),
                ), // .take(1),
            )
            .exec()
            .await?
            .ok_or_else(|| DBError::DoesNotExist(format!("order {}", order_id)))?;

        let (app, sub_app) = {
            let app = order.app.take().ok_or_else(|| {
                DBError::SQLFailed(format!("failed fetch app on order {}", order_id))
            })?;
            let sub_app = order.sub_app.take().ok_or_else(|| {
                DBError::SQLFailed(format!("failed fetch sub_app on order {}", order_id))
            })?;
            (*app, *sub_app)
        };

        let charges = order.charges.take().unwrap_or_default();

        Ok((order, charges, app, sub_app))
    }

    pub async fn load_charge_from_db(
        prisma_client: &crate::prisma::PrismaClient,
        charge_id: &str,
    ) -> Result<
        (
            crate::prisma::charge::Data,
            Option<crate::prisma::order::Data>,
            Vec<crate::prisma::refund::Data>,
            crate::prisma::app::Data,
            Option<crate::prisma::sub_app::Data>,
        ),
        DBError,
    > {
        let mut charge = prisma_client
            .charge()
            .find_unique(crate::prisma::charge::id::equals(charge_id.into()))
            .with(
                crate::prisma::charge::order::fetch().with(crate::prisma::order::sub_app::fetch()),
            )
            .with(crate::prisma::charge::app::fetch())
            .with(crate::prisma::charge::refunds::fetch(vec![]).order_by(
                crate::prisma::refund::created_at::order(prisma_client_rust::Direction::Desc),
            ))
            .exec()
            .await?
            .ok_or_else(|| DBError::DoesNotExist(format!("charge {}", charge_id)))?;
        let mut order = charge
            .order
            .take()
            .ok_or_else(|| {
                DBError::SQLFailed(format!("failed fetch order on charge {}", &charge_id))
            })?
            .map(|order| *order);
        let app = charge.app.clone().ok_or_else(|| {
            DBError::SQLFailed(format!("failed fetch app on charge {}", &charge_id))
        })?;
        let sub_app = match order {
            Some(ref mut order) => {
                let sub_app = order.sub_app.take().ok_or_else(|| {
                    DBError::SQLFailed(format!("failed fetch sub_app on charge {}", &charge_id))
                })?;
                Some(*sub_app)
            }
            None => None,
        };
        let refunds = charge.refunds.take().unwrap_or_default();
        Ok((charge, order, refunds, *app, sub_app))
    }

    pub async fn load_channel_params_from_db(
        prisma_client: &crate::prisma::PrismaClient,
        app_id: Option<&str>,
        sub_app_id: Option<&str>,
        channel: &str,
    ) -> Result<crate::prisma::channel_params::Data, DBError> {
        let mut where_params = vec![crate::prisma::channel_params::channel::equals(
            channel.to_string(),
        )];
        if let Some(app_id) = app_id {
            where_params.push(crate::prisma::channel_params::app_id::equals(Some(
                app_id.to_string(),
            )));
        }
        if let Some(sub_app_id) = sub_app_id {
            where_params.push(crate::prisma::channel_params::sub_app_id::equals(Some(
                sub_app_id.to_string(),
            )));
        }
        let channel_params = prisma_client
            .channel_params()
            .find_first(where_params)
            // .find_unique(crate::prisma::channel_params::sub_app_id_channel(
            //     sub_app_id.to_string(),
            //     channel.to_string(),
            // ))
            .exec()
            .await?
            .ok_or_else(|| {
                DBError::DoesNotExist(format!(
                    "channel_params {:?} for app {:?} / sub app {:?}",
                    channel, app_id, sub_app_id
                ))
            })?;
        Ok(channel_params)
    }
}

pub use db::*;
