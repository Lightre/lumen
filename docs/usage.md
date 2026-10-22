# Using Lumen

- Click **Open image** or press **Ctrl+O** to select a photo.
- Launch `lumen.exe "C:\Photos\example.jpg"`, drop a photo onto the executable in Explorer, or select the executable through Windows **Open with**. Only the first file argument is opened; without an argument, Lumen starts with an empty view. Paths containing spaces must be quoted when launching from a terminal.
- Drop an image file onto the window to open it. When dropping multiple files, only the first is opened.
- Use **Previous** / **Next** or the **Left** / **Right** arrow keys to navigate the current folder. Files are sorted naturally by name (`photo1`, `photo2`, `photo10`), ignoring case.
- Navigation stops at the first and last file. Unreadable images are skipped, and the counter includes all matching files, including unreadable ones. The folder list refreshes when an image is opened explicitly; new files added afterward appear on the next explicit open.
- The status bar shows the file name and image dimensions. Cancelling a selection or failing to open a new file preserves the current image.
- JPEG, PNG, WebP, GIF, BMP, and TIFF files are supported. Animated GIFs play automatically and respect their loop count. Press **Space** to pause or resume; after a finite animation completes, **Space** restarts it. Animated PNG and WebP are shown as static images, and color profiles are not managed.
- TIFF files with `.tif` or `.tiff` extensions display the first image only, including multi-image TIFFs. **Left** / **Right** navigate between files in the folder. 16-bit source data is displayed through the existing 8-bit preview; ICC color profiles are not managed.
- Scroll over the image area to zoom around the pointer, or use the **−** / **+** buttons. Drag with the left mouse button to pan when the image exceeds the view.
- **Fit** or **Home** resets zoom and pan while keeping the current rotation. **100%** maps one source image pixel to one physical screen pixel, accounting for display scaling. Double-click the image area to toggle between Fit and 100%.
- Use **Rotate left** / **Rotate right** or **Shift+R** / **R** to turn the view by 90°. Rotation recenters the image and keeps the zoom mode. It does not modify the file; the status bar continues to show the loaded image's original dimensions.
- Use **Full screen** or **F11** for a view without toolbars. **Esc** or **F11** exits full screen. Folder navigation, rotation, zoom, and pan remain available in full screen.
- Opening another image resets rotation, zoom, and pan to Fit. Zoom is limited to 1600%; the minimum is 1% or the fit scale, whichever is smaller. Fit never enlarges images beyond 100%.

Images load in the background. Only one open operation runs at a time. The current limits are 256 MiB per file and 32 megapixels per image, with additional decoder resource limits. GIF decoded-frame storage is limited to 128 MiB and 1,000 frames; GIFs that exceed either limit are rejected rather than truncated. Reduced previews retain the original image dimensions and are indicated in the status bar; zooming does not restore detail removed from a preview.

