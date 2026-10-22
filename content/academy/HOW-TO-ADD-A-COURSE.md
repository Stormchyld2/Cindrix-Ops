# How to write a course

A course is a folder. Copy this whole folder, rename the copy (use letters, numbers and dashes, no
spaces, for example `home-gardening`), take away the leading underscore, and put it in the
`academy` folder on your drive. The Academy shows it the next time it is opened. Folders that start
with `_` are ignored, which is why this template does not show up.

```
academy/
  home-gardening/                 the course (the folder name is its id)
    course.md                     title, summary, author, version
    01-getting-started/           a module (a chapter); folders sort by name
      _module.md                  the module's title and summary
      01-choose-a-spot.md         a lesson
      02-prepare-the-soil.md
    02-growing/
      ...
```

## course.md

```
---
title: Home gardening
summary: Growing food at home with little money and no internet.
author: Your name
version: 1
license: Who may copy it (for example "CC BY 4.0" or "All rights reserved")
---
```

Raise `version` when you change the course. If two copies of a course are found (say, one on your
laptop and a newer one on a drive), the higher version is used.

## A lesson

```
---
title: Choose a spot
minutes: 6
status: draft
source: Where the facts come from
---
Plain paragraphs. Leave a blank line between them.

## A heading starts with two # signs
- A bullet starts with a dash
1. A numbered step starts with a number and a dot
> A note box starts with a > sign. Use **two stars** for bold.

## Check yourself
? A question starts with a question mark
+ The one right answer starts with a plus
- Each wrong answer starts with a dash
> Why the right answer is right (optional)
```

Rules the Academy checks:

- Every lesson needs a `title:` and some text.
- Every question needs at least two answers and exactly one `+` answer.
- **Lessons never give medicine amounts (doses).** A lesson that does is left out. Say to follow the
  label or to ask a doctor or pharmacist.
- `status: reviewed` means a qualified person has checked the lesson. Anything else shows as a
  draft, with a plain notice, so readers know.

If a file is left out, the Academy tells you which one and why (at the bottom of its screen). The
rest of the course still works.

## Making a course from a book in your library
Open the book in the Library (Shelf) and read it a chapter at a time. For each chapter make a module folder, and for each
part of the chapter a lesson file: write the main points **in your own words** (do not copy pages), add the book's name and
chapter to `source:` and finish with three questions under `## Check yourself`. The `_template-course` folder shows the shape.
You can also ask the assistant to summarise a chapter, then edit the result before saving it.
Put the course folder in `user/academy` (Settings, Your own cards and plans, opens it). Only copy what you have the right to use.
