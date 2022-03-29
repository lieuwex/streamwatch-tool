use crate::common::*;

use std::path::Path;

use tokio::process::Command;

use chrono::{DateTime, Duration, Local};

use sqlx::SqliteConnection;

use anyhow::{Context, Result};

async fn ffmpeg_trim(
    settings: &Settings,
    old_path: &Path,
    new_path: &Path,
    seconds: f32,
) -> Result<()> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&["-v", "quiet"]);
    cmd.arg("-stats");
    cmd.arg("-i");
    cmd.arg(old_path);
    cmd.arg("-ss");
    cmd.arg(seconds.to_string());
    cmd.arg("-c");
    cmd.arg("copy");
    cmd.arg(new_path);

    settings.print(|| format!("{:?}", cmd));

    if !settings.dry_run {
        cmd.spawn()?.wait().await?.exit_ok()?;
    }

    Ok(())
}

pub async fn trim_lw(
    conn: &mut SqliteConnection,
    settings: &Settings,
    streams_folder: &Path,
) -> Result<()> {
    let items: Vec<(i64, String, f32, i32)> = sqlx::query!("select id,filename,game_features.start_time,count(*) over (partition by id) as count from streams join game_features on game_features.stream_id = streams.id where game_features.game_id = 7")
        .map(|row| (row.id, row.filename, row.start_time, row.count.unwrap()))
        .fetch_all(&mut *conn)
        .await
        .context("Failed to retrieve information from database")?;
    let total = items.len();

    for (i, (stream_id, filename, start_time, total_count)) in items.into_iter().enumerate() {
        if total_count > 1 {
            settings.print(|| {
                format!(
                    "[{}/{}] skipping {} because we have more than 1 Einde LW in the stream",
                    i + 1,
                    total,
                    stream_id,
                )
            });
            continue;
        }

        let start_time = start_time - 1.0;
        assert!(start_time >= 0.0);

        let old_stream_path = Path::new(streams_folder).join(filename);
        assert!(old_stream_path.exists());
        let old_chat_path = old_stream_path.with_extension("txt.zst");
        let old_yaml_path = old_stream_path.with_extension("yaml");

        let (old_time, old_time_type): (DateTime<Local>, DateType) =
            match parse_filename(&old_stream_path) {
                Some((d, typ)) => (d, typ),
                None => {
                    settings.print(|| {
                        format!("couldn't get date for {:?}. Skipping.", old_stream_path)
                    });
                    continue;
                }
            };
        let new_time: DateTime<Local> =
            old_time + Duration::milliseconds((start_time * 1000.0) as i64);

        let (new_file_base, rename_extra_files) = match old_time_type {
            DateType::Full => (new_time.format("%Y-%m-%d %H:%M:%S").to_string(), true),
            DateType::DateOnly => (new_time.format("%Y-%m-%d").to_string() + "_NEW", false),
        };
        let map_path = |old: &Path| {
            let extension = {
                let old = old.file_name().unwrap().to_str().unwrap();
                old.split_once('.').unwrap().1
            };
            let res = old.with_file_name(format!("{}.{}", new_file_base, extension));
            assert!(!res.exists());
            res
        };
        let new_stream_path = map_path(&old_stream_path);
        let new_chat_path = map_path(&old_chat_path);
        let new_yaml_path = map_path(&old_yaml_path);

        settings.print(|| {
            format!(
                "[{}/{}] {:?} -> {:?}",
                i + 1,
                total,
                old_stream_path,
                new_stream_path
            )
        });
        ffmpeg_trim(settings, &old_stream_path, &new_stream_path, start_time)
            .await
            .context("Failed to trim the video file")?;

        if old_chat_path.exists() && rename_extra_files {
            rename(settings, &old_chat_path, &new_chat_path)
                .await
                .context("Failed to rename chat file")?;
        }
        if old_yaml_path.exists() && rename_extra_files {
            rename(settings, &old_yaml_path, &new_yaml_path)
                .await
                .context("Failed to rename yaml file")?;
        }

        // remove the LW from the database for this stream.
        sqlx::query!(
            "DELETE FROM game_features WHERE stream_id = ? AND game_id = 7",
            stream_id,
        )
        .execute(&mut *conn)
        .await
        .context("Failed to remove LW game features from database")?;

        // update the time for all the game features
        sqlx::query!(
            "UPDATE game_features SET start_time=max(start_time-?,0) WHERE stream_id = ?",
            start_time,
            stream_id,
        )
        .execute(&mut *conn)
        .await
        .context("Failed to update game features in database")?;

        // update the time for all watch progress
        sqlx::query!(
            "UPDATE stream_progress SET time=max(time-?,0) WHERE stream_id = ?",
            start_time,
            stream_id,
        )
        .execute(&mut *conn)
        .await
        .context("Failed to update stream progress in database")?;

        match old_time_type {
            DateType::Full => {
                // update the filename in the database
                {
                    let new_stream_filename =
                        new_stream_path.file_name().unwrap().to_str().unwrap();
                    let timestamp = new_time.timestamp();
                    sqlx::query!(
                        "UPDATE streams SET filename=?,ts=?,duration=duration-? WHERE id = ?",
                        new_stream_filename,
                        timestamp,
                        start_time,
                        stream_id,
                    )
                    .execute(&mut *conn)
                    .await
                    .context("Failed to update database to use new stream file")?;
                }

                remove_file(settings, &old_stream_path)
                    .await
                    .context("Failed to remove old stream file")?;
            }
            DateType::DateOnly => {
                rename(settings, &new_stream_path, &old_stream_path)
                    .await
                    .context("Failed to rename new stream file back to old name")?;
            }
        }
    }

    Ok(())
}
