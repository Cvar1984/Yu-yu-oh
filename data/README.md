# data

| File | Contents | In git |
| --- | --- | --- |
| `cards.json` | The 374 cards of LOB, MRD and SRL | yes, from the data bundle |
| `banlist.json` | Copy limits (0, 1 or 2) from the owner's game, kept by hand | yes |
| `collection.json` | The owner's cards and copy counts | yes, from the data bundle |
| `decks/*.json` | The owner's deck and the prototype's best decks, with results in `notes` | yes |
| `../images/full`, `../images/small` | One JPEG per card id | no: Konami-owned, fetched locally |

Card id is the key everywhere: it is the card's official password, and every
other file refers to it. Names in `banlist.json` and `collection.json` are
comments for humans; the loader checks ids only.

`../tools/refresh_cards.py` rebuilds `cards.json` and downloads missing images.
`../reference/python_prototype/` holds the Python simulator the handoff's
findings came from; its rules were simplified, so treat its numbers as
hypotheses.
