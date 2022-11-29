use crate::{
    db::entity::{post, user},
    deliverer::Deliverer,
    error::{Error, Result},
    mapping::IntoActivityPub,
    state::Zustand,
};
use futures_util::{stream, StreamExt};
use phenomenon_model::ap::{Activity, PUBLIC_IDENTIFIER};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// TODO: Enforce via semaphore or something
const MAX_CONCURRENT_REQUESTS: usize = 10;

#[derive(Deserialize, Serialize)]
pub struct DeliveryContext {
    post_id: Uuid,
}

pub async fn run(state: &Zustand, deliverer: &Deliverer, ctx: DeliveryContext) -> Result<()> {
    let Some((post, Some(user))) = post::Entity::find_by_id(ctx.post_id)
        .find_also_related(user::Entity)
        .one(&state.db_conn)
        .await?
    else {
        return Ok(());
    };

    // TODO: Resolve follower collection
    let activity: Activity = todo!();
    let user_ids = activity
        .rest
        .to
        .iter()
        .filter(|url| *url != PUBLIC_IDENTIFIER)
        .chain(
            activity
                .rest
                .cc
                .iter()
                .filter(|url| *url != PUBLIC_IDENTIFIER),
        )
        .map(String::as_str);

    let inbox_stream = stream::iter(user_ids).then(|ap_id| async {
        let user = state.fetcher.fetch_actor(ap_id).await?;
        Ok::<_, Error>(user.inbox_url)
    });

    tokio::pin!(inbox_stream);

    // TODO: Run this concurrently
    // TODO: Don't error out if a single inbox failed to resolve
    while let Some(inbox) = inbox_stream.next().await.transpose()? {
        deliverer.deliver(&inbox, &user, &activity).await?;
    }

    Ok(())
}
