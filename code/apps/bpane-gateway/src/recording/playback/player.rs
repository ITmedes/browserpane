use super::model::{RecordingPlaybackError, SessionRecordingPlaybackManifest};

pub(super) fn build_player_html(
    manifest: &SessionRecordingPlaybackManifest,
) -> Result<String, RecordingPlaybackError> {
    let manifest_json = serde_json::to_string(manifest)?;
    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>BrowserPane Recording Playback</title>
  <style>
    :root {{
      color-scheme: light;
      font-family: "SF Mono", "Menlo", monospace;
      background: linear-gradient(160deg, #f2efe7, #ddd7cb);
      color: #1b1a17;
    }}
    body {{
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      padding: 32px;
    }}
    main {{
      width: min(960px, 100%);
      background: rgba(255, 255, 255, 0.82);
      backdrop-filter: blur(12px);
      border: 1px solid rgba(0, 0, 0, 0.08);
      border-radius: 24px;
      box-shadow: 0 28px 80px rgba(38, 32, 21, 0.14);
      padding: 24px;
    }}
    h1 {{
      margin-top: 0;
      font-size: 20px;
      letter-spacing: 0.04em;
      text-transform: uppercase;
    }}
    video {{
      width: 100%;
      border-radius: 16px;
      background: #111;
    }}
    .meta {{
      display: flex;
      gap: 12px;
      flex-wrap: wrap;
      margin: 16px 0;
      font-size: 13px;
    }}
    .meta span {{
      padding: 6px 10px;
      background: rgba(0, 0, 0, 0.05);
      border-radius: 999px;
    }}
    ol {{
      padding-left: 20px;
      margin: 0;
      display: grid;
      gap: 6px;
      font-size: 13px;
    }}
    button {{
      margin-right: 8px;
    }}
  </style>
</head>
<body>
  <main>
    <h1>BrowserPane Recording Playback</h1>
    <video id="player" controls autoplay></video>
    <div class="meta" id="summary"></div>
    <p>
      <button id="prev">Previous</button>
      <button id="next">Next</button>
    </p>
    <ol id="segments"></ol>
  </main>
  <script>
    const manifest = {manifest_json};
    const player = document.getElementById('player');
    const summary = document.getElementById('summary');
    const segmentList = document.getElementById('segments');
    const prev = document.getElementById('prev');
    const next = document.getElementById('next');
    let index = 0;

    function renderSummary() {{
      summary.innerHTML = '';
      [
        `state: ${{manifest.state}}`,
        `segments: ${{manifest.included_segment_count}} / ${{manifest.segment_count}}`,
        `duration_ms: ${{manifest.included_duration_ms}}`,
        `bytes: ${{manifest.included_bytes}}`
      ].forEach((value) => {{
        const node = document.createElement('span');
        node.textContent = value;
        summary.appendChild(node);
      }});
    }}

    function renderSegments() {{
      segmentList.innerHTML = '';
      manifest.segments.forEach((segment, segmentIndex) => {{
        const item = document.createElement('li');
        item.textContent = `${{segment.sequence}}. ${{segment.recording_id}}`;
        if (segmentIndex === index) {{
          item.style.fontWeight = '700';
        }}
        segmentList.appendChild(item);
      }});
    }}

    function loadSegment(segmentIndex) {{
      if (!manifest.segments.length) {{
        return;
      }}
      index = Math.max(0, Math.min(segmentIndex, manifest.segments.length - 1));
      player.src = manifest.segments[index].file_name;
      renderSegments();
    }}

    player.addEventListener('ended', () => {{
      if (index + 1 < manifest.segments.length) {{
        loadSegment(index + 1);
        player.play().catch(() => {{}});
      }}
    }});
    prev.addEventListener('click', () => loadSegment(index - 1));
    next.addEventListener('click', () => loadSegment(index + 1));

    renderSummary();
    renderSegments();
    loadSegment(0);
  </script>
</body>
</html>
"#
    ))
}
