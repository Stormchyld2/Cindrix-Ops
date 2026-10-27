# Content plan: real books where we can, our own writing where we cannot

Decision (Oct 2026): bundle the actual book or manual whenever its license lets us sell it on a drive. Where the good book is commercial or non-commercial only, we do not bundle it. We write our own short, original material for the topic, and list the real book under "Recommended reading" (a plain list in the Library, no files).

Our own writing is marked "Cindrix original · draft · pending review". Medical, food-safety and anything that could hurt someone gets a review before release. Nothing here gives medicine or insulin doses.

License status: **PD** = US government work or public domain. **Verify** = I believe it is free to include but the license text must be checked before it goes on a sold drive. **No** = do not bundle.

| Topic | Real source to bundle | Status | Our own writing |
|---|---|---|---|
| First aid | US Army TC 4-02.1 First Aid | PD | |
| Survival | US Army FM 21-76-1 Survival, Evasion, and Recovery (1999), cover says "Approved for public release; distribution is unlimited" | PD (checked) | |
| Home canning and preserving | USDA Complete Guide to Home Canning (NCHFP) | PD, verify | |
| Rainwater | Texas Water Development Board, Texas Manual on Rainwater Harvesting | Verify | basic rainwater card |
| Nuclear and chemical | Nuclear War Survival Skills (Kearny, ORNL); Army NBC manuals | Verify; PD | |
| Mental health | SAMHSA/NCTSN Psychological First Aid Field Operations Guide | PD, verify | `stress-and-morale.md` |
| Vehicle basics | Army TM 9-8000 Principles of Automotive Vehicles | PD, verify | |
| Home repair | Army and HUD manuals; Stack Exchange DIY ZIM (CC BY-SA) | Verify | |
| Encyclopedias | Wikipedia, WikiMed (Kiwix, CC BY-SA) | Yes | |
| Recipes | Wikibooks Cookbook (CC BY-SA), USDA recipes | Yes | |
| Radio | FCC Part 97 rules, public ham exam question pools | PD | radio starter card |
| Sanitation | Free disaster sanitation PDFs (check each license) | Verify | `sanitation.md` |
| Generators and CO | CDC, FEMA | PD | `generator-and-co-safety.md` |
| Morale | | | `stress-and-morale.md` |
| Kids | | | `activities-with-kids.md` (ideas list only, no kids shelf) |
| Off-grid power | DOE and NREL guides | PD | solar and battery basics card |

## Recommended reading (listed, never bundled)
Survival Medicine Handbook; Where There Is No Doctor and Where There Is No Dentist (Hesperian, non-commercial license); Ball Complete Book of Home Preserving; Humanure Handbook; Foxfire; All New Square Foot Gardening; Gaia's Garden; Prepper's Long-Term Survival Guide; Deep Survival; ARRL ham license manual.

## Left out on purpose
Political or economic forecasting. (Defense and weapons books were listed here in an earlier draft without the owner's say-so; the owner decided on 2026-10-27 that they are wanted, so they are no longer left out.) A home-safety shelf (fire, carbon monoxide, securing the house) covers the safe part.

## How the app uses it
Real books go in as Kiwix archives or PDFs/MD in the library folder, with a Sources screen row for each (title, license, link). Our own pieces are Markdown in `content/`. Each file carries its source in the answer's "Sources" line.


## License findings (Oct 18, 2026)

- **Where There Is No Doctor / Where There Is No Dentist (Hesperian): cannot bundle as is.** Hesperian's terms allow free non-commercial copying, but "any digital format" and "commercial purposes" need written permission. Selling a drive that contains them is commercial and digital. Option: ask Hesperian for written permission (they may say no or ask for a fee). Until then they are "Recommended reading" only.
- **US Army FM 3-05.70 Survival (2002): do not bundle.** The copy I checked is stamped "Distribution authorized to U.S. Government agencies and their contractors only". Use **FM 21-76-1** (1999 multiservice manual, approved for public release) and, if its own cover says public release, the older FM 21-76 (1992).
- **Rule for every government manual:** read the distribution statement on the cover page of the exact PDF. Only "Approved for public release" goes on the drive. The build script should fail the build for a PDF whose first pages contain "Distribution authorized to U.S. Government agencies" or "Distribution restricted".
- TC 4-02.1 First Aid: the cover statement was not confirmed. Check it the same way.
- Project Gutenberg books may be sold, as long as we follow its terms (remove the Gutenberg trademark or keep the license text).

## Shelf and reader (design)

Idea: a Kindle-like shelf inside Library, mockups `Shelf` and `Reader`.

- **Two kinds of book.** Scanned or designed PDFs (Army manuals, USDA guides) keep their pages, because first-aid and knot pictures matter. They open in a bundled PDF viewer (pdf.js, Apache 2.0) that is read-only. Our own pieces and Gutenberg books are Markdown, shown as reflowable text with adjustable size.
- **Storage rule.** PDF stays an export-only format for anything the user writes. Books on the shelf are read-only content, the one place PDFs are stored. Notes, bookmarks, highlights and reading position are JSON files in `user/`.
- **Search and AI.** At build time each PDF's text is extracted page by page (pdftotext, no OCR) into a page-numbered Markdown index. The assistant searches the index, answers from it and cites "Book, page N". The citation opens the reader on that page. Scanned books with no text layer need OCR at build time, so skip those or OCR them with Tesseract (Apache 2.0).
- **Shelf metadata.** `shelf/index.json`: title, author, license, source URL, category, cover PNG (first page rendered at build time), file, page count.
- **Reader features.** Contents, search in book, text size, night mode, bookmarks, highlights, notes, jump to page, "Ask about this page", resume where you left off.
- **My own books.** "Add my own" copies a user's file into `my-library/`.
- **Android.** WebView on Android cannot show PDFs by itself, but pdf.js works in both.

## Adding your own books (built)

The Shelf has "+ Add a book" and accepts drag and drop. It takes PDF, Markdown and text files, up to 400 MB each, and refuses anything else or a file that says PDF but is not one. The same file twice is refused. Files are copied to `user/my-library/books/` under a safe name (`u1.pdf`), the list is `books.csv`, and bookmarks, notes and the reading position are `user/marks.csv`.

When a PDF is opened the reader reads its text page by page in the background, which makes search inside the book work, and saves it once to `user/my-library/text/` so the assistant can use the book. Answers cite the book and page. The medication rule still applies to everything the assistant says, including answers drawn from the person's own books.

Known limits: scanned PDFs without a text layer can be read but not searched; PDFs that rely on special fonts or Asian-language character maps may show with substitute fonts until those files are bundled with the app; highlights are not built yet; books are not deleted by the app until the person chooses Remove.

## Archives reader and sharing (built)

Library has three tabs: Shelf, Archives, Emergency cards. Archives opens the Kiwix reader (the bundled `kiwix-serve`, MIT/GPL program) inside the app on this computer only (127.0.0.1), so .zim files are read in a built-in browser without internet. No Docker is needed.

Sharing: off by default. When turned on (with a confirmation), the library program listens on the local network, port 8787 when free, so phones and tablets on the same Wi-Fi or hotspot can open it in their own browser. The app tells the person the address. It is read-only and only the archives are served, not the shelf, inventory, family page or the assistant. The footer changes from "This app only talks to itself on this device" to a sharing notice while it is on. Windows will show a firewall prompt the first time.

Which archives to bundle: Wikipedia, WikiMed, Wikibooks, Wikisource and Project Gutenberg (public domain), and Stack Exchange archives such as DIY, Home Improvement, Outdoors and Gardening (CC BY-SA, attribution required). Skip anything under a non-commercial license (for example wikiHow and iFixit). The build script should read each ZIM's license from its metadata and refuse non-commercial ones.

Defense, firearms, military and weapons-maintenance books are allowed (owner, 2026-10-27), as long as each has a clear license or is public domain. One limit stays in the shipped lists and download scripts, and it is Claude's own: step-by-step manuals for building explosives, booby traps or improvised munitions are not listed or fetched by them. The owner can still add their own files to the Shelf.

## Save a website from inside the app (design, not built)

Goal: the person pastes a web address, the app saves that site for offline reading.

- **Crawler in the app.** Written in Rust with a small HTTP client. It fetches pages on the same site only, follows links to a depth and page limit the person sets, saves the HTML and the images, CSS and scripts the page needs, rewrites links to point at the saved copies, obeys robots.txt, waits between requests, and shows progress with a Stop button. Works for ordinary and documentation-style sites. Sites that build their pages with JavaScript, need a login or block crawlers will come out partly or fully empty. A headless-browser mode could be added later.
- **Where it is saved.** First version: a folder per site in `library/sites/<name>/`, served read-only by the same local library server, listed in the Archives tab, with its text also written out as Markdown so the assistant can search it. A true `.zim` file is the better long-term format because other Kiwix readers can open it, but writing one means implementing the ZIM format (or bundling libzim), which is a second, larger step.
- **Honesty.** This is the one feature that reaches the internet on purpose. It only runs when the person starts it, the footer shows "Internet in use: saving a website" while it runs, and the sentence "This app only talks to itself on this device" is replaced for that time.
- **Permission.** The person is saving a private copy for themselves. The screen says so, and says that the site's own terms apply. We never ship such copies on drives we sell.
- **Size.** A limit in pages and megabytes is required, with a default, so a link-heavy site cannot fill a stick.
