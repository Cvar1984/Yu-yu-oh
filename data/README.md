# data

| File | Contents | In git |
| --- | --- | --- |
| `cards.json` | The 440 cards of the owner's game: LOB, MRD and SRL (374) plus 66 EXTRA cards; `in_game_index` says which list each sits under | yes, from the data bundle |
| `extra_cards.txt` | Names of the 66 EXTRA cards, for `refresh_cards.py` | yes |
| `banlist.json` | Copy limits (0, 1 or 2) from the owner's game, kept by hand; every player follows them | yes |
| `collection.json` | Save A: the owner's cards and copy counts | yes, from the data bundle |
| `collection_save_b.json` | Save B | yes, from the data bundle |
| `decks/*.json` | The owner's decks, the prototype's best decks and six enemy decks, with results in `notes`; `for` says whose cards a deck uses | yes |
| `../images/full`, `../images/small` | One JPEG per card id | no: Konami-owned, fetched locally |

Card id is the key everywhere: it is the card's official password, and every
other file refers to it. Names in `banlist.json` and `collection.json` are
comments for humans; the loader checks ids only.

`../tools/refresh_cards.py` rebuilds `cards.json` and downloads missing images.
`../reference/python_prototype/` holds the Python simulator the handoff's
findings came from; its rules were simplified, so treat its numbers as
hypotheses.
