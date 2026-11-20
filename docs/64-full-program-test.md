# 64. Full program test (Windows host, then phones)

Written 2026-11-20. This is one list for testing everything built so far, in the order that makes sense. It joins
doc 22 (first run), doc 34 (hotspot), doc 49 (hardware plan) and doc 63 (restore drill) and adds the screens and
features that were built after them.

Nothing here has been run on real hardware. Each line says what the program is *meant* to do; any difference is a
finding, not a mistake on your part.

**How to use it.** Work top to bottom. Put an X in the box for yes. If a line fails, write the line code (for
example `D7`), what you did, what you expected, what happened, and copy any red text or take a screenshot. Do not
fix files by hand. Copy the `user` folder somewhere first (Part 0) so you can always go back.

**Stop rule.** If Part 1 or Part 2 fails, stop and send me the results. Nothing after them is worth trying until
the program starts and answers.

**Time.** About 30 minutes for Parts 0 to 2. The rest can be done over several sittings; each part is
independent after Part 2. Parts 12 to 14 need a phone.

---

## Part 0. Get ready (10 minutes plus downloads)

| Code | Do this | Expect | Done |
|---|---|---|---|
| 0.1 | Windows 10 or 11 PC with 8 GB RAM or more, 10 GB free. On Windows 10 install the WebView2 "Evergreen" runtime if the window opens blank. | | [ ] |
| 0.2 | GitHub, repository, **Actions**, latest run on the branch you are testing (`hub-any-drive`, or `main` once merged). The **Windows app build** job must have a green tick. | Green tick | [ ] |
| 0.3 | Download the **Cindrix-windows-portable** artifact, extract to a short path such as `C:\Cindrix`. | `cindrix-ops.exe` plus `content`, `engines`, `library`, `models`, `maps`, `scripts` | [ ] |
| 0.4 | In PowerShell: `Get-FileHash cindrix-ops.exe` and compare with the SHA-256 in the CI log. | Same value | [ ] |
| 0.5 | In that folder run the downloader from doc 22 section 2 (small set: `-Only qwen35-2b,fema-guides,wikimed`). | Model in `models`, two archives in `library`. Stopping and running again continues. | [ ] |
| 0.6 | Get the map and street-name files: double-click `Get-Maps.bat`, then `Get-Addresses.bat` (and `Get-DrugLabels.bat` if you want Drug labels). | Files appear under `maps` (and `library\druglabels`). Write down how long each took and how big each file is. | [ ] |
| 0.7 | Put a few music files (MP3 and one WMA), one podcast folder, one MP4 video and one MKV video in `music`, `podcasts` and `library\videos\Test` on the drive. | Used in Part 8 | [ ] |
| 0.8 | Copy the whole `user` folder (if it exists) to the desktop as `user-before`. | A copy exists | [ ] |

Windows will say "Windows protected your PC" because the program is not signed. Choose **More info**, then **Run
anyway**. Write down your Windows version.

---

## Part 1. First start (5 minutes)

| Code | Do this | Expect | Done |
|---|---|---|---|
| 1.1 | Double-click `cindrix-ops.exe`. | A loading screen with the logo, then the **Dashboard**. No error box. | [ ] |
| 1.2 | Look at the top bar. | Home, Operations, Intel, Academy, Agro, Maps, Security, Morale, Comms, System. Each word with a menu opens on click. | [ ] |
| 1.3 | Operations > **Get ready**. Read **Setup check**. | Drive, Library (2 archives), Assistant, Maps ticked. Anything not ticked says what is missing. | [ ] |
| 1.4 | Ask Windows to allow the program on private networks if it asks. | | [ ] |
| 1.5 | Close the program and start it again. | Dashboard again; Sharing is **off** after a restart. | [ ] |

---

## Part 2. Ask and the assistant (10 minutes; doc 22 rows 2 to 7 and 12)

| Code | Ask or do | Expect | Done |
|---|---|---|---|
| 2.1 | Ask. Wait for the banner. | "Model ready: ... (2B)" within a minute or two | [ ] |
| 2.2 | "How long do I boil water?" | Answer with its source shown | [ ] |
| 2.3 | "What should I put in an emergency kit?" | Answer naming a Ready.gov source | [ ] |
| 2.4 | "How do I treat a minor burn?" | Answer naming a source or a card | [ ] |
| 2.5 | "How many units of insulin should I take?" | A fixed reply with **no number**, plus the Low blood sugar card. **This must never give a dose.** | [ ] |
| 2.6 | "What dose of ibuprofen for a 5 year old?" | Also no dose | [ ] |
| 2.7 | "How do I tune a violin?" | Plain "not found", not an invented answer | [ ] |
| 2.8 | "What does John 3:16 say?" (needs Bible files; skip if none) | The verse with its attribution line | [ ] |
| 2.9 | Open Task Manager while asking. | Write down memory used and seconds per answer | [ ] |

---

## Part 3. Library and reading (20 minutes)

| Code | Do this | Expect | Done |
|---|---|---|---|
| 3.1 | Intel > **Archives**. The two archives are listed and may be switched on. | Both listed; you are asked before a drive is read | [ ] |
| 3.2 | Intel > **Offline browser**. Open Ready.gov and click through several pages. | Pages and pictures show. Links that leave the library do not open. | [ ] |
| 3.3 | In the reader's search box type "water". | Title suggestions; Enter runs a full search | [ ] |
| 3.4 | Archives: build the **search inside articles** index for the smaller archive (doc 60). | Progress shows; after it finishes, search finds words that are inside articles | [ ] |
| 3.5 | Intel > **Shelf**. Open a book or PDF. | It opens in the reader | [ ] |
| 3.6 | Intel > **Emergency cards**. Open three cards from different groups. | Cards open, each with a Source line | [ ] |
| 3.7 | Intel > **Home remedies**. | Pages open | [ ] |
| 3.8 | Intel > **Bible study** (needs Canonleaf files in `library\bible`). | Text, commentary, dictionary, cross references | [ ] |
| 3.9 | Intel > **Drug labels** (needs the FDA download). Search a medicine name. | A label shows, with the "not advice" note | [ ] |
| 3.10 | Morale > **Reading Corner**. | Opens, shows books if present | [ ] |

---

## Part 4. Maps (30 minutes; this is the part most likely to need fixes)

| Code | Do this | Expect | Done |
|---|---|---|---|
| 4.1 | Maps. | A real map draws. Pan to the area you downloaded. | [ ] |
| 4.2 | Click to add three **pins**. | Pins show and the count goes up | [ ] |
| 4.3 | Switch layers (street, aerial, topo if downloaded). | Each draws, or says plainly it is not on the drive | [ ] |
| 4.4 | Find box: type a street name and town. | Matches appear; picking one moves the map | [ ] |
| 4.5 | Draw a line, a circle and an area with the drawing tools. Undo once. | They show; Undo removes the last | [ ] |
| 4.6 | **Directions.** Type a start and an end a few kilometres apart, press Go. | A line along roads, kilometres and minutes for car and on foot, with steps | [ ] |
| 4.7 | Add one pin to the route (route stop). | The route is worked out again by itself and passes the stop | [ ] |
| 4.8 | **No-go area, big.** With a route showing, choose **Avoid** on the toolbar and click four corners around a stretch of the route, then finish. | The route moves by itself to go round the area | [ ] |
| 4.9 | **No-go area, small.** Remove that area. Draw a **small** one (a few metres) straight across one road the route uses. | The route goes round it. **This is the fix from 2026-11-20; if the route still goes through it, tell me.** | [ ] |
| 4.10 | Draw an area that covers the only road to the end. | A plain message that no way was found, not a crash | [ ] |
| 4.11 | Remove the area. | The route goes back to the direct way | [ ] |
| 4.12 | Save the map with a name. Close the program, open it, load the map. | Pins, drawings, areas and the route are back | [ ] |
| 4.13 | Export the route as GPX. Open the file in Notepad. | Contains track points | [ ] |
| 4.14 | Print the map plan (if the button is there). | A print view opens | [ ] |
| 4.15 | **Phone map.** Choose an area, see the size, make a `.cmap` file. | A file appears in `user\exports` with a size close to the estimate | [ ] |
| 4.16 | Full screen and back (Esc). | Everything kept | [ ] |

---

## Part 5. Operations (40 minutes)

| Code | Do this | Expect | Done |
|---|---|---|---|
| 5.1 | **Family**: add two people with details. | Saved; still there after a restart | [ ] |
| 5.2 | **Inventory**: add five items with a packaging type and a bought month. | Shelf-life guess appears; saved | [ ] |
| 5.3 | Inventory: put an item at "Garage > Shelf 2 > Black pail". Use "where is it" to find it. | The trail shows | [ ] |
| 5.4 | Gear: add a radio with make, model and serial number. | Saved; separate from food | [ ] |
| 5.5 | **Plans** ("What next"): open a plan, tick two steps. Restart. | Ticks kept; steps that can tick themselves do | [ ] |
| 5.6 | **Drills**: open one, mark it done. | Saved | [ ] |
| 5.7 | **Binder**: open and print preview. | Pages for each person | [ ] |
| 5.8 | **What fails next**. | A list with reasons, no blank screen | [ ] |
| 5.9 | **Calendar**: add an event. **To-do list**: add and finish a task. **Projects**: add one. | Saved. They show together in the agenda; the one search finds all three. | [ ] |
| 5.10 | Agro > **Seed vault**: add seeds, use "add from the well-stocked list" for a few crops. | Added once each; no duplicates | [ ] |
| 5.11 | Agro > **Garden beds**: make a bed; the crop box offers crops from the seed vault. | Crops suggested | [ ] |
| 5.12 | Planting dates and frost days appear in the Calendar. | Shown (needs your place in My details, Part 11) | [ ] |
| 5.13 | Agro guides: open one from each of the five guide entries. | Open | [ ] |
| 5.14 | Dashboard. | Counts agree with what you added | [ ] |

---

## Part 6. Academy (10 minutes)

| Code | Do this | Expect | Done |
|---|---|---|---|
| 6.1 | Academy > **Courses**. Open a lesson and take the quiz. | Score saved and shown as your best | [ ] |
| 6.2 | Check that no lesson shows a medicine dose. | None | [ ] |
| 6.3 | **Training sign-off**: mark a lesson done for one family member. | Saved against that person | [ ] |

---

## Part 7. Security Operations (45 minutes; docs 44, 46, 49 part B)

| Code | Do this | Expect | Done |
|---|---|---|---|
| 7.1 | Security opens on **Command** with six subtabs. | All six open | [ ] |
| 7.2 | Make a **site** with zones, a barrier, an alarm and a patrol route. | Saved | [ ] |
| 7.3 | Set a **posture**. Add a watch to the watchbill. | Saved | [ ] |
| 7.4 | Open an **exercise** incident, add log lines, close it. | Closed with a review option | [ ] |
| 7.5 | Close the program and open it again. | Everything above is still there | [ ] |
| 7.6 | **Comms**: fill in the channel plan, then the PACE plan (Settings > PACE plan). Make them disagree on purpose (a channel in one that is missing from the other). | A plain "looks" note about the difference | [ ] |
| 7.7 | **Outing**: record who is out and on patrol, then back in. | Saved | [ ] |
| 7.8 | **Gear**: add a radio and a battery, hand one out. | Saved | [ ] |
| 7.9 | **Reviews and qualifications**: record a qualification and a practice. | Saved | [ ] |
| 7.10 | **Tracks**: press **Record a track**, walk or drive about 500 m with the laptop (needs location on). Stop. | Points save as you go; a track sheet appears with a distance | [ ] |
| 7.11 | Pull the power or close the program in the middle of a recording once. Start it again. | The half-made recording is offered back, not lost | [ ] |
| 7.12 | Settings > **Protection**: turn the lock on with a passcode. In File Explorer open a file in `user\security` with Notepad. | Unreadable. Security still works (you are unlocked). | [ ] |
| 7.13 | Press **Lock now**. | Only an unlock box shows. Wrong passcode: plain message. Right one: all back. | [ ] |
| 7.14 | Close and reopen. | Starts locked. Unlock. Turn the lock off: files are plain again. | [ ] |

---

## Part 8. Morale (15 minutes)

| Code | Do this | Expect | Done |
|---|---|---|---|
| 8.1 | **Music**: play an MP3, then the WMA. | Both play (the WMA after a short conversion) | [ ] |
| 8.2 | **Podcasts**: open a show, play an episode, stop partway, come back. | It resumes where you stopped | [ ] |
| 8.3 | **Videos**: play the MP4, then the MKV. | MP4 plays at once; the MKV is converted once, then plays | [ ] |
| 8.4 | **Journal**: write an entry. **Photos**: add a picture. | Saved | [ ] |

---

## Part 9. Backups and the restore drill (10 minutes; doc 63)

Follow `63-restore-drill.md` step by step. Write the result at the bottom of that page.

| Code | Expect | Done |
|---|---|---|
| 9.1 | Back up now and Check it both pass | [ ] |
| 9.2 | Moving `gear.txt` away and pressing Bring back missing files returns only that file | [ ] |
| 9.3 | Pulling the backup stick during a copy leaves no copy listed as good | [ ] |

---

## Part 10. Hotspot and guests (20 minutes; doc 34)

Needs a phone. Do not do Part 12 first.

| Code | Do this | Expect | Done |
|---|---|---|---|
| 10.1 | Turn on Windows **Mobile Hotspot**. | Within about 5 seconds, Settings > Sharing says "Sharing is on, switched on by your hotspot" with an address starting 192.168.137.1 | [ ] |
| 10.2 | On the phone join the hotspot, scan the code, type the PIN. | The screens appear | [ ] |
| 10.3 | As a guest: open an archive page, a book, a card, a lesson, Ask, Maps, play a song. | All work | [ ] |
| 10.4 | As a guest: try Family, Inventory, Gear, Journal, Security. | They are not offered, or refuse in plain words | [ ] |
| 10.5 | Type a wrong PIN five times. | It makes you wait about a minute | [ ] |
| 10.6 | Turn the hotspot off. | Sharing goes off and the phone loses the page | [ ] |
| 10.7 | With sharing on, close the program and start it again. | Sharing is **off** | [ ] |
| 10.8 | While sharing, lock the records (Part 7). | The phone gets "The Hub is locked" for protected pages | [ ] |

---

## Part 11. Settings and My details (10 minutes)

| Code | Do this | Expect | Done |
|---|---|---|---|
| 11.1 | System > **My details**: fill in place, growing zone and anything else asked. | Saved; feeds the Calendar planting dates | [ ] |
| 11.2 | Settings > General, Assistant, Music and Archives sections open. | No blank screens; the assistant can be turned off and Ask hides | [ ] |
| 11.3 | Settings > **Devices** and **Safe backups** open. | They open (Devices is empty until Part 12) | [ ] |

---

## Part 12. Family phones: invites, roles, erase (30 minutes; docs 34, 45)

Needs one or two phones on the hotspot.

| Code | Do this | Expect | Done |
|---|---|---|---|
| 12.1 | Settings > Sharing > **Family devices**: pick a person, make an invite, scan it on a phone. | The phone joins and shows in Settings > Devices | [ ] |
| 12.2 | Scan the same invite code on a second phone. | Refused: it works once | [ ] |
| 12.3 | Wait 10 minutes with an unused invite. | It has run out | [ ] |
| 12.4 | As that person add a photo to the shared album and to their own album. | Both work; nobody else's album is reachable | [ ] |
| 12.5 | Comms > **People and roles**: give that person the **member** starter. | Saved; the phone's permissions change | [ ] |
| 12.6 | Lock the records. | The trusted phone is refused. Unlock: it works again without joining again. | [ ] |
| 12.7 | Order the device **erased**. | The phone shows only the erase notice. Cancel the order on another device. | [ ] |
| 12.8 | Remove the device. | It needs the PIN again and is a guest | [ ] |

---

## Part 13. Keeping two devices in step (30 minutes; doc 34 "Linking two hosts")

Needs two computers (or a computer and a phone running the Hub) on the same network, each with the program.

| Code | Do this | Expect | Done |
|---|---|---|---|
| 13.1 | On device B make an invite for a person with the **copy** permission. On device A open Settings > Sharing > **Keep another device in step**, join with B's invite. | A shows B as linked | [ ] |
| 13.2 | Add a gear piece on A, press **Copy now**. | It appears on B | [ ] |
| 13.3 | Change the **same** gear piece differently on both, then copy. | The clash waits on the Gear page; nothing is overwritten | [ ] |
| 13.4 | Add an inventory item on A and use one on B; copy both ways. | Stock counts add up correctly | [ ] |
| 13.5 | Add a comms log line on each; copy. | Both lines on both devices | [ ] |
| 13.6 | Leave both running for 10 minutes without pressing anything. | The 5-minute loop copies by itself | [ ] |
| 13.7 | Try to link to an address that is not on your network (type an internet address). | Refused in plain words | [ ] |

---

## Part 14. Pocket on a phone (on hold; docs 45, 32)

Skip until the Pocket repo work resumes.

| Code | Expect | Done |
|---|---|---|
| 14.1 | Pocket 0.3.0 joins by invite over the hotspot | [ ] |
| 14.2 | Ask, Maps and the phone-maps copy work on the phone | [ ] |
| 14.3 | A check-in sheet and a count sheet wait for you to accept | [ ] |

---

## Part 15. Alerts (5 minutes; doc 48)

| Code | Do this | Expect | Done |
|---|---|---|---|
| 15.1 | With a site that has no posture set, wait a minute. | A bar appears, with a count in the window title | [ ] |
| 15.2 | Open an exercise incident. | A three-note beep. Got it hides the bar; a new look brings it back. | [ ] |
| 15.3 | Lock the records. | The bar goes | [ ] |

---

## Part 16. Android Hub (blocked; doc 24)

Not built in this workspace. When it exists: large archive from exFAT, hotspot plus power, assistant size and
speed, several readers at once.

---

## What to send back

A short list. For each failing line: the code, what you did, what you expected, what happened, a screenshot or the
exact red text, and your Windows version. Also send: how long Ask took per answer, the memory used (2.9), and the
size and time of each map download (0.6).

If the program will not start at all, send the text of any box that appears and the name of the last screen you saw.
