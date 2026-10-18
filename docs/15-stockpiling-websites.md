# 15. Stockpiling websites as .zim files (works today)

Do this on a computer with internet. The result is a .zim file you copy into the drive's `library` folder. The Archives tab shows it, and the assistant can search and quote it (answers say which archive the passage came from).

## With Zimit (Kiwix's free tool)

1. Install Docker Desktop (needs Windows 10/11 with virtualization on).
2. Open a terminal and run, changing the folder and address:

   `docker run -v C:\zims:/output ghcr.io/openzim/zimit zimit --seeds https://example.com --name example-site --pageLimit 2000`

   - `--seeds` is the starting address (several can be separated by commas; the first becomes the home page).
   - `--name` becomes the file name.
   - `--pageLimit` caps how many pages are saved, so a big site cannot fill the drive.
   - `--scopeExcludeRx "regex"` skips parts of a site (repeatable).
   - `docker run ghcr.io/openzim/zimit zimit --help` lists everything.
3. When it finishes, copy `C:\zims\example-site.zim` into the drive's `library` folder and restart the app.

These commands come from the Zimit project's page. I have not run them. Zimit loads pages in a real browser, so it handles sites that build their pages with JavaScript, which a simple downloader cannot.

## Rules for yourself
- A private copy for your own use is one thing. The site's terms and copyright still apply, and we never ship saved copies of other people's sites on drives we sell.
- Pick sites with a clear license when you can (government, universities, open projects).
- Check the file size before copying. A Go drive has a 100 GB content budget.

## How the assistant reads archives
The assistant sends your question to the library reader, takes the best-matching articles, and then picks the section of each article that matches best (not just the introduction). It quotes only those sections and names the archive and article. A .zim file made without a search index cannot be searched this way. Zimit and Wikipedia-style archives normally include one.
