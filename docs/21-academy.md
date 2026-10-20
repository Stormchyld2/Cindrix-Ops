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

## What the program does
- Shows modules and lessons, with a progress bar.
- Reads a lesson and offers a short quiz (answers are shown in a random order). The score is saved on request in `user/academy.csv` (date, lesson, score, total); the best score is shown.
- Lessons are found by their id in the loaded course. The interface never builds a file path.

## Tests
Core tests check the parser, bad quizzes (no right answer, two right answers), loading order, the results file (a damaged file is reported and never overwritten), and that every shipped lesson has minutes, a named source, at least 3 questions and no dosing text. The screenshot run opens a lesson, takes the quiz and saves the score.

## Content today
Seven modules, 21 lessons: the first 72 hours, survival mindset, shelter/fire/water, navigation and signaling, field first aid, evade/resist/escape for civilians, and weeks to months. **All are drafts**, written from public-domain US government manuals and public guidance, and need review by a qualified person (especially first aid) before the `status` is changed to `reviewed`. They are a start, not a finished SERE school: more modules can be added by adding folders.

## Not built yet
Linking the assistant's answers to lessons, pictures and diagrams, printing a lesson, and a certificate or checklist view.
