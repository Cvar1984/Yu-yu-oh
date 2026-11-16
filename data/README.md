# data

| File | Contents | In git |
| --- | --- | --- |
| `cards.json` | The 374 cards of LOB, MRD and SRL | yes, from the data bundle |
| `banlist.json` | Copy limits (0, 1 or 2) from the owner's game, kept by hand | yes |
| `collection.json` | The owner's cards and copy counts | yes, from the data bundle |
| `decks/*.json` | Decks in the format `ygo check-deck` reads | yes |
| `../images/full`, `../images/small` | One JPEG per card id | no: Konami-owned, fetched locally |

Card id is the key everywhere: it is the card's official password, and every
other file refers to it. Names in `banlist.json` and `collection.json` are
comments for humans; the loader checks ids only.
