use rand::Rng;

pub fn generate_id(prefix: &str) -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let mut rng = rand::thread_rng();
    let number: u64 = rng.gen_range(10000000000..100000000000);
    format!("{}{}{}", prefix, timestamp, number)
}

pub fn notify_url(charge_id: &str) -> String {
    let charge_notify_origin = std::env::var("CHARGE_NOTIFY_ORIGIN").unwrap();
    format!("{}/notify/charges/{}", charge_notify_origin, charge_id)
    // "https://notify.pingxx.com/notify/charges/ch_101240601691280343040013";
}

pub fn refund_notify_url(charge_id: &str, refund_id: &str) -> String {
    let charge_notify_origin = std::env::var("CHARGE_NOTIFY_ORIGIN").unwrap();
    format!(
        "{}/notify/charges/{}/refunds/{}",
        charge_notify_origin, charge_id, refund_id
    )
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
        let order = prisma_client
            .order()
            .find_unique(crate::prisma::order::id::equals(order_id.to_string()))
            .with(crate::prisma::order::sub_app::fetch())
            .with(crate::prisma::order::app::fetch())
            .with(
                crate::prisma::order::charges::fetch(vec![
                    // crate::prisma::charge::is_valid::equals(true)
                ])
                .order_by(crate::prisma::charge::created_at::order(
                    prisma_client_rust::Direction::Desc,
                )), // .take(1),
            )
            .exec()
            .await?
            .ok_or_else(|| DBError::DoesNotExist(format!("order {}", order_id)))?;

        let (app, sub_app) = {
            let order = order.clone();
            let app = order.app.ok_or_else(|| {
                DBError::SQLFailed(format!("failed fetch app on order {}", order_id))
            })?;
            let sub_app = order.sub_app.ok_or_else(|| {
                DBError::SQLFailed(format!("failed fetch sub_app on order {}", order_id))
            })?;
            (*app, *sub_app)
        };

        let charges = order.charges.clone().unwrap_or_default();

        Ok((order, charges, app, sub_app))
    }

    pub async fn load_charge_from_db(
        prisma_client: &crate::prisma::PrismaClient,
        charge_id: &str,
    ) -> Result<
        (
            crate::prisma::charge::Data,
            crate::prisma::order::Data,
            crate::prisma::app::Data,
            crate::prisma::sub_app::Data,
        ),
        DBError,
    > {
        let charge = prisma_client
            .charge()
            .find_unique(crate::prisma::charge::id::equals(charge_id.into()))
            .with(
                crate::prisma::charge::order::fetch()
                    .with(crate::prisma::order::sub_app::fetch())
                    .with(crate::prisma::order::app::fetch()),
            )
            .exec()
            .await?
            .ok_or_else(|| DBError::DoesNotExist(format!("charge {}", charge_id)))?;
        let order = charge
            .order
            .clone()
            .ok_or_else(|| {
                DBError::SQLFailed(format!("failed fetch order on charge {}", &charge_id))
            })?
            .ok_or_else(|| {
                DBError::DoesNotExist(format!("order not found on charge {}", &charge_id))
            })?;
        let app = order.app.clone().ok_or_else(|| {
            DBError::SQLFailed(format!("failed fetch app on charge {}", &charge_id))
        })?;
        let sub_app = order.sub_app.clone().ok_or_else(|| {
            DBError::SQLFailed(format!("failed fetch sub_app on charge {}", &charge_id))
        })?;
        Ok((charge, *order, *app, *sub_app))
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
