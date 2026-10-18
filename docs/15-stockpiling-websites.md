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

## Saving a website from inside the app (built, not yet run by a person)

Archives tab → **Save a website**. Give it an address, pick how much (the whole site, one folder,
or a page limit) and a size limit, and press the button. The program:

1. reads the site's `robots.txt` and obeys it, and waits about 0.6 seconds between requests;
2. follows the site's own links (never to other sites), fetching pictures, style sheets and fonts
   from any host the pages use;
3. drops scripts, rewrites every link to point inside the archive, and writes a `.zim` file into
   the library folder, with a front page that lists everything saved;
4. keeps a plain-text copy in `library/web-text/` so the assistant can answer from it (the writer
   cannot build Kiwix's own full-text index, so Kiwix's search box finds saved sites by title only).

Limits, stated plainly: sites that need JavaScript to show their content come out empty; logins are
not supported; videos are skipped if over 25 MB per file; archives are not compressed, so they are
larger than Zimit's. If a site matters and it comes out wrong, use Zimit (above), which drives a real
browser. The core logic is tested with a made-up website; the CI job `zim-check` opens a sample
archive with the real `zimcheck` and `kiwix-serve`.

## Finding the drive

A Cindrix drive has a small file, `cindrix-drive.json`, in its top folder (or in a `Cindrix` folder
there). The program looks for it on every drive letter (or mount point) at start and every few
seconds until it finds one, so the drive can be any letter on any computer. If the program itself
runs from the drive, that drive is used. The build script writes the file (`-Edition go|vault`).
