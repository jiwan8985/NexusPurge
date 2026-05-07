# UI Screenshot Regression Checklist

Capture the following screens after UI changes:

- Profile modal: S3 settings open, CDN settings open, Purge policy open.
- Progress dialog: active transfer, canceled transfer, Purge status badge, CDN URL list.
- Log panel: Log tab, Queue tab, Purge history tab.
- Dry-run preview: summary, changed files, Purge Preview.
- Remote panel states: disconnected, empty bucket, CDN configured empty path.

Recommended filenames:

```text
screenshots/profile-modal.png
screenshots/progress-dialog.png
screenshots/log-panel.png
screenshots/dry-run-preview.png
screenshots/remote-empty.png
```

Verification:

```text
[ ] Text is not clipped at desktop width.
[ ] Buttons show focus rings when tabbed.
[ ] File rows can be selected with keyboard.
[ ] Empty states match connection/CDN state.
```
