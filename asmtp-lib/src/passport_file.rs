use anyhow::{Context as _, Result};
use keynesis::passport::block::{Block, BlockSlice};
use std::{io::ErrorKind, path::Path};
use tokio::{
    fs,
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt as _},
};

pub async fn export_passport_blocks_to<P>(
    blocks: impl IntoIterator<Item = BlockSlice<'_>>,
    path: P,
) -> Result<()>
where
    P: AsRef<Path>,
{
    let file = fs::OpenOptions::new()
        .read(false)
        .write(true)
        .append(false)
        .truncate(true)
        .create(true)
        .open(path.as_ref())
        .await
        .with_context(|| {
            format!(
                "Failed to export passport to path {}",
                path.as_ref().display()
            )
        })?;

    export_passport_blocks(blocks, file).await.with_context(|| {
        format!(
            "Failed to export passport to path {}",
            path.as_ref().display()
        )
    })
}

pub async fn import_passport_blocks_from<P>(path: P) -> Result<Vec<Block>>
where
    P: AsRef<Path>,
{
    let file = fs::OpenOptions::new()
        .read(true)
        .write(false)
        .append(false)
        .truncate(false)
        .create(false)
        .open(path.as_ref())
        .await
        .with_context(|| {
            format!(
                "Failed to import passport from path {}",
                path.as_ref().display()
            )
        })?;

    import_passport_blocks(file).await.with_context(|| {
        format!(
            "Failed to import passport from path {}",
            path.as_ref().display()
        )
    })
}

/// write the passport to the given `output`
pub async fn export_passport_blocks<O>(
    blocks: impl IntoIterator<Item = BlockSlice<'_>>,
    mut output: O,
) -> Result<()>
where
    O: AsyncWrite + Unpin,
{
    for block in blocks.into_iter() {
        write_block(&mut output, block).await?;
    }

    Ok(())
}

pub async fn import_passport_blocks<I>(mut input: I) -> Result<Vec<Block>>
where
    I: AsyncRead + Unpin,
{
    let mut blocks = Vec::new();

    while let Some(block) = read_block(&mut input).await? {
        blocks.push(block);
    }

    Ok(blocks)
}

async fn write_block<O>(mut output: O, block: BlockSlice<'_>) -> Result<()>
where
    O: AsyncWrite + Unpin,
{
    let len = block.as_ref().len() as u64;
    output
        .write_all(&len.to_be_bytes())
        .await
        .context("Failed to write block to the output")?;
    output
        .write_all(block.as_ref())
        .await
        .context("Failed to write block to the output")?;

    Ok(())
}

async fn read_block<I>(mut input: I) -> Result<Option<Block>>
where
    I: AsyncRead + Unpin,
{
    let size = match input.read_u64().await {
        Ok(len) => len as usize,
        Err(error) => {
            if error.kind() == ErrorKind::UnexpectedEof {
                return Ok(None);
            } else {
                return Err(error).context("Failed to read block from the passport");
            }
        }
    };
    let mut bytes = vec![0; size];
    input
        .read_exact(&mut bytes)
        .await
        .with_context(|| format!("Failed to read {} bytes of the blocks", size))?;

    let block = BlockSlice::try_from_slice(&bytes)
        .context("Invalid passport's block")?
        .to_block();

    Ok(Some(block))
}
