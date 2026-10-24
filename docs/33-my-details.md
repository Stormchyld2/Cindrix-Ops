# 33. My details

Settings > **My details** holds the things about the household that several parts of the program can use.
All of it is optional.

- **Home** and **places to bug out to**: a name and an address, town, ZIP code or position in any form.
  "Find it" looks the position up with the map's own finder and keeps it.
- **Who lives here**: adults, children, babies, small and large pets (for the stock plan).
- **People to reach and places to meet**: name, how to reach them, notes.
- **Needs, skills and notes**: free text. Not for medicine doses.

It is one plain file, `user/profile.txt` (one `key: value` line per fact), saved with
`fsio::backup_then_write` so an earlier copy is kept. The code is `core/src/profile.rs` (tested there).
It is host-only: the sharing server never gives it to guests.

On the Maps screen a **My places** box appears when a home or bug-out position is saved. A place is on the
map only while it is shown (Show / Hide); Start and End put its position into the Directions boxes.

**Used by:** the stock plan (Inventory > How much should I have? > **Use my details** fills in who lives here), and Ask: when a question is about home, bug-out places, evacuating or the household (`profile::question_is_personal`), the plain sentences from `profile::for_assistant` (phone numbers left out) go to the assistant as its first source, named "My details". Guests never get this.
