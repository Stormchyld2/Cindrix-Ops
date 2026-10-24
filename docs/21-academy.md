# 21. Academy

A text course built into the program, the same in Go, Vault and Hub. It is small (about 120 KB of text for the first seven modules) so it costs the drives nothing.

## Where it lives
`content/academy/<NN-module>/<NN-lesson>.md`, one folder per module with a `_module.md` (title, summary). Lessons are plain text with a small header:

```
---
title: Water comes first
minutes: 8
status: draft
source: FEMA/Ready.gov water guidance
---
Paragraphs. "## " headings, "- " bullets, "1. " steps, "> " note boxes, **bold**.

## Check yourself
? Question
+ the right answer
- a wrong answer
> why
```

Anyone can add or fix a lesson with a text editor. A lesson that does not parse is left out; it never breaks the course.

## Courses
The Academy is a small program that reads course folders. A course is one folder inside an `academy` folder:

```
academy/<course>/course.md         title, summary, author, version, license
academy/<course>/<NN-module>/_module.md
academy/<course>/<NN-module>/<NN-lesson>.md
```

- The first screen is a grid of **course cards** (title, summary, author, modules, lessons, hours, progress, and "not yet reviewed" for drafts). Choose a card to go inside the course; "All courses" goes back, and a lesson's back button returns to its course.
- Courses can sit in more than one Academy folder (the program's own, each library drive). If the same course id is in two places, the higher `version` wins.
- The older layout (modules straight under `academy`) is one course, "survival-basics", with the same lesson ids, so saved scores still match.
- Folders starting with `_` or `.` are skipped. `content/academy/_template-course/` is a copy-and-edit starting point.
- **No medicine doses.** A lesson that contains a dose is left out and listed under "course files left out" (the same check the assistant uses, `safety::output_contains_dose`). A file that does not parse is left out the same way; the rest of the course still works.

## What the program does
- Shows modules and lessons, with a progress bar.
- Reads a lesson and offers a short quiz (answers are shown in a random order). The score is saved on request in `user/academy.csv` (date, lesson, score, total); the best score is shown.
- Lessons are found by their id in the loaded course. The interface never builds a file path.

## Tests
Core tests check the parser, bad quizzes (no right answer, two right answers), loading order, the results file (a damaged file is reported and never overwritten), and that every shipped lesson has minutes, a named source, at least 3 questions and no dosing text. The screenshot run opens a lesson, takes the quiz and saves the score.

## Content today
One course (Survival basics), seven modules, 21 lessons: the first 72 hours, survival mindset, shelter/fire/water, navigation and signaling, field first aid, evade/resist/escape for civilians, and weeks to months. **All are drafts**, written from public-domain US government manuals and public guidance, and need review by a qualified person (especially first aid) before the `status` is changed to `reviewed`. They are a start, not a finished SERE school: more modules can be added by adding folders.

## Not built yet
Nothing is listed as missing from the first plan. (Ask already links an answer to the lesson, card, plan, book page or guide page it came from.)

On Training sign-off, each followed course has **Certificate to print** (once signed off) or **Checklist to print** (lesson boxes ticked for the lessons passed, with a line to sign).

A lesson has a **Print lesson** button (black on white, without the quiz); "Save as PDF" in the print window keeps a copy.

## Pictures in lessons

A line of its own in a lesson, `![what it shows](name.png)`, shows the picture `name.png` kept in the same folder as the lesson file (png, jpg, gif, webp or svg, up to 1.5 MB). Only plain file names count (no folders, no web addresses); anything else stays ordinary text. A missing picture shows "Picture not found" and the lesson still works. Code: `academy::image_file`, `image_data_url` (tested). The picture's words are checked for medicine doses like the rest of the lesson.

## Course packs

A course pack is one text file (`.ccourse`) holding a course's folders, lesson files and pictures (`core/src/coursepack.rs`, tested). **Academy > Share my courses as a pack** writes the courses in `user/academy` to `user/exports/my-courses.ccourse`. To add a pack, put the file in `user/academy-packs` and press **Install course packs**. The whole pack is checked first (plain file names only, allowed file types, size limits); a course that is already installed is never overwritten. Installed packs are renamed to end in `.done`. Both commands are host-only.
