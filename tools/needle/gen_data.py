"""Generate Needle fine-tune data for ROM filename classification.

Corpus strategy (spike): curated famous-title seed lists per platform + the
repo's builtin Redump titles, with synthetic region/disc/scene tag mangling.
Production swap-in: real Redump DATs parsed the same way (labels identical).

Labels mirror src-tauri/src/classifier/jev.rs + redump.rs exactly:
  platform: psx | saturn | dreamcast | segacd | pcecd
  region:   USA | Europe | Japan | World   (from the filename tag)
  disc_number: "single" | "1".."8"
  is_multidisc: bool
"""
import json, random, re, hashlib, os

random.seed(20260926)

# ---------------------------------------------------------------------------
# Seed title lists (famous/representative, high-confidence per platform)
# ---------------------------------------------------------------------------
TITLES = {
    "psx": [
        "Final Fantasy VII", "Final Fantasy VIII", "Final Fantasy IX", "Metal Gear Solid",
        "Castlevania: Symphony of the Night", "Chrono Cross", "Resident Evil", "Resident Evil 2",
        "Resident Evil 3: Nemesis", "Silent Hill", "Gran Turismo", "Gran Turismo 2",
        "Crash Bandicoot", "Crash Bandicoot 2: Cortex Strikes Back", "Crash Bandicoot 3: Warped",
        "Crash Team Racing", "Spyro the Dragon", "Spyro 2: Ripto's Rage", "Spyro: Year of the Dragon",
        "Tekken 3", "Tekken 2", "Soul Blade", "Xenogears", "Vagrant Story", "Vandal Hearts",
        "Suikoden", "Suikoden II", "Lunar 2: Eternal Blue", "Parasite Eve", "Parasite Eve II",
        "Dino Crisis", "Dino Crisis 2", "Street Fighter Alpha 3", "Alpha 3" ,
        "Tony Hawk's Pro Skater 2", "Tony Hawk's Pro Skater", "MediEvil", "MediEvil 2",
        "Ape Escape", "Bushido Blade", "Einhander", "Ehrgeiz", "Brave Fencer Musashi",
        "Legend of Mana", "Front Mission 3", "Final Fantasy Tactics", "Tactics Ogre",
        "Rayman", "Oddworld: Abe's Oddysee", "Oddworld: Abe's Exoddus", "Tomba!",
        "Tomba! 2", "Klonoa: Door to Phantomile", "R4: Ridge Racer Type 4", "Ridge Racer",
        "Wipeout 3", "Wipeout XL", "Colin McRae Rally", "Driver", "Driver 2",
        "Syphon Filter", "Syphon Filter 2", "Syphon Filter 3", "Tenchu: Stealth Assassins",
        "Tenchu 2: Birth of the Stealth Assassins", "Fear Effect", "Fear Effect 2: Retro Helix",
        "Legacy of Kain: Soul Reaver", "Blood Omen: Legacy of Kain", "Legacy of Kain: Soul Reaver 2",
        "Digimon World", "Monster Rancher", "Pokemon Trading Card Game" ,
        "Breath of Fire III", "Breath of Fire IV", "Wild Arms", "Wild Arms 2", "Star Ocean: The Second Story",
        "Grandia", "Threads of Fate", "Mega Man Legends", "Mega Man 8", "Mega Man X4",
        "Mega Man X5", "Mega Man X6", "R-Type Delta", "Einhander" , "G Darius", "Darius Gaiden",
        "DoDonPachi", "Raiden Project", "Gradius Gaiden", "Silent Bomber", "The Misadventures of Tron Bonne",
        "Jumping Flash!", "Jumping Flash! 2", "Intelligent Qube", "Kula World", "Pandemonium",
        "Ace Combat 3: Electrosphere", "Ace Combat 2", "Rogue Squadron" ,
        "Twisted Metal", "Twisted Metal 2", "Twisted Metal 4", "Vigilante 8", "Vigilante 8: Second Offense",
        "Destruction Derby", "Rollcage", "Croc: Legend of the Gobbos", "Gex: Enter the Gecko",
        "Duke Nukem: Time to Kill", "Duke Nukem: Land of the Babes", "Quake II", "Unreal Tournament",
        "Need for Speed: High Stakes", "Need for Speed III: Hot Pursuit", "Test Drive 4",
        "Jet Moto", "Jet Moto 2", "Cool Boarders 2", "1080° Snowboarding" ,
        "Poy Poy", "Bomberman World", "Bust-a-Move 4", "Puzzle Fighter II Turbo",
        "Persona", "Persona 2: Eternal Punishment", "Rhapsody: A Musical Adventure",
        "Hoshigami: Ruining Blue Earth", "Kartia: The Word of Fate", "Azure Dreams", "Torneko: The Last Hope",
        "Dragon Warrior VII", "Legend of Dragoon", "Alundra", "Alundra 2", "Brigandine",
        "Koudelka", "Shadow Madness", "SaGa Frontier", "SaGa Frontier 2", "Front Mission Alternative",
        "Carnage Heart", "Armored Core", "Armored Core: Project Phantasma", "Armored Core: Master of Arena",
        "King's Field", "King's Field II", "Echo Night", "Overblood", "Hellnight",
        "Clock Tower", "Clock Tower II: The Struggle Within", "Martian Gothic: Unification",
        "Echo Night 2" , "Popolocrois Monogatari", "Tales of Destiny", "Tales of Eternia",
        "Thousand Arms", "Jade Cocoon: Story of the Elvendays", "Monster Seed", "Racing Lagoon",
        "The Adventure of Little Ralph", "Ace Combat", "Pop'n Tanks", "Devil Dice", "I.Q.: Intelligent Qube",
    ],
    "saturn": [
        "Panzer Dragoon", "Panzer Dragoon Zwei", "Panzer Dragoon Saga", "Nights into Dreams",
        "Sega Rally Championship", "Virtua Fighter 2", "Virtua Fighter Remix", "Fighters Megamix",
        "Fighting Vipers", "Last Bronx", "Guardian Heroes", "Radiant Silvergun",
        "Dragon Force", "Shining Force III", "Shining the Holy Ark", "Albert Odyssey",
        "Burning Rangers", "Virtua Cop", "Virtua Cop 2", "Sega Touring Car Championship",
        "Manx TT Super Bike", "Hang-On GP 96", "Daytona USA", "Daytona USA: Championship Circuit Edition",
        "Sega Ages: OutRun", "Space Harrier", "After Burner II", "Galactic Attack",
        "Layer Section", "Batsugun", "Sokyugurentai", "Battle Garegga", "Soukyugurentai",
        "Darius Gaiden", "G Darius" , "Powerslave", "Duke Nukem 3D", "Quake",
        "Exhumed", "Alien Trilogy", "Resident Evil", "Wipeout", "Wipeout 2097",
        "Sega Rally" , "Bugs Bunny in Double Trouble", "Clockwork Knight", "Clockwork Knight 2",
        "Astal", "Mr. Bones", "Nicheristmas Nights" , "Christmas Nights", "DecAthlete",
        "Virtua Athlete 2000", "World Series Baseball", "World Series Baseball 98",
        "NHL Powerplay 96", "FIFA 96", "FIFA 97", "Pebble Beach Golf Links",
        "Baku Baku Animal", "Puyo Puyo Sun", "Puyo Puyo Tsu", "Columns III",
        "Golden Axe: The Duel", "Guardian Heroes Deluxe" , "Legend of Oasis", "Story of Thor 2",
        "Beyond Oasis", "Policenauts", "Snatcher" , "Lunar: Silver Star Story Complete",
        "Magic Knight Rayearth", "Azel Panzer Dragoon RPG" , "Baroque", "Tactics Ogre",
        "Mystaria: The Realms of Mystery", "Blazing Heroes" , "Shining Wisdom", "Shining Force II",
        "Shining in the Darkness" , "Galaxy Fight", "Waku Waku 7", "X-Men: Children of the Atom",
        "Marvel Super Heroes", "Street Fighter Alpha 2", "Street Fighter Collection",
        "Real Bout Fatal Fury Special", "Fatal Fury 3", "King of Fighters 96", "King of Fighters 97",
        "Samurai Shodown IV", "Vampire Savior", "Darkstalkers 3" , "Megaman 8",
        "Mega Man 8" , "Saturn Bomberman", "Doom", "Hexen", "Heretic",
        "Amok", "Scorcher", "Crocel" , "N2O: Nitrous Oxide" , "Panzer Front" ,
        "Deep Fear", "Enemy Zero", "D", "D2", "Nightmare Creatures", "Lunacy",
        "Myst", "Riven", "Discworld", "Blazing Dragons", "Pandemonium",
        "Gex", "Bug!", "Bug Too!", "Sonic 3D Blast", "Sonic R", "Sonic Jam",
        "Sega Genesis Collection" , "NiGHTS into Dreams", "Puyo Puyo Box" ,
    ],
    "dreamcast": [
        "Shenmue", "Shenmue II", "Soulcalibur", "Sonic Adventure", "Sonic Adventure 2",
        "Jet Set Radio", "Crazy Taxi", "Crazy Taxi 2", "Power Stone", "Power Stone 2",
        "Marvel vs. Capcom 2", "Capcom vs. SNK 2", "Street Fighter III: 3rd Strike",
        "Skies of Arcadia", "Grandia II", "Phantasy Star Online", "Phantasy Star Online Ver. 2",
        "Resident Evil Code: Veronica", "Resident Evil 2", "Resident Evil 3: Nemesis",
        "Metropolis Street Racer", "Sega GT", "Ferrari F355 Challenge", "Le Mans 24 Hours",
        "Test Drive Le Mans" , "Tokyo Xtreme Racer", "Tokyo Xtreme Racer 2", "Tokyo Highway Challenge" ,
        "Daytona USA 2001", "Sega Rally 2", "Virtua Fighter 3tb", "Fighting Vipers 2",
        "Last Blade 2" , "The Last Blade: Final Edition", "Garou: Mark of the Wolves",
        "King of Fighters 99" , "King of Fighters 2000", "King of Fighters 2001", "King of Fighters 2002",
        "Samurai Shodown V" , "Guilty Gear X", "Rival Schools 2" , "Project Justice",
        "Dead or Alive 2", "Tech Romancer", "Plasma Sword", "Star Wars Episode I: Racer",
        "Starlancer", "Unreal Tournament", "Quake III Arena", "Half-Life",
        "MDK 2", "Armada", "Gauntlet Legends", "Rayman 2: The Great Escape",
        "Legacy of Kain: Soul Reaver", "Tomb Raider: The Last Revelation", "Tomb Raider Chronicles",
        "Ecco the Dolphin: Defender of the Future", "Seaman", "Samba de Amigo",
        "Space Channel 5", "ChuChu Rocket!", "Puyo Puyo Fever" , "Bomberman Online",
        "Toy Commander", "Blue Stinger", "Carrier", "Illbleed", "D2",
        "Code Veronica" , "Sword of the Berserk: Guts' Rage", "Omikron: The Nomad Soul",
        "Headhunter", "Shenmue Chapter 1: Yokosuka" , "Azurik" , "Max Pool" ,
        "Virtua Tennis", "Virtua Tennis 2", "Tennis 2K2" , "NBA 2K", "NBA 2K1", "NBA 2K2",
        "NFL 2K1", "NFL 2K2", "NHL 2K2", "World Series Baseball 2K1", "World Series Baseball 2K2",
        "Ready 2 Rumble Boxing: Round 2", "Rush 2049", "San Francisco Rush 2049",
        "Hydro Thunder", "4 Wheel Thunder", "Crazy Taxi" , "18 Wheeler: American Pro Trucker",
        "Sega Marine Fishing", "Sega Bass Fishing", "Sega Bass Fishing Duel" ,
        "Zero Gunner 2", "Ikaruga", "Mars Matrix", "Giga Wing 2", "Trigger Heart Exelica" ,
        "Border Down", "Chaos Field", "Psyvariar 2" , "Shikigami no Shiro II", "Castle of Shikigami 2",
        "Puyo Pop" , "Snatcher" , "Time Stalkers", "Evolution: The World of Sacred Device",
        "Evolution 2: Far Off Promise", "Seventh Cross", "Elemental Gimmick Gear",
        "Silver", "Knightmare's" , "Wetrix+", "Pen Pen TriIcelon", "Expendable",
        "Re-Volt", "Vigilante 8: Second Offense", "Twisted Metal 4" ,
        "Zombie Revenge", "House of the Dead 2", "Confidential Mission", "Death Crimson OX",
        "Nakayoshi to Issho" , "Roommania 203", "Shenmue the Movie" ,
    ],
    "segacd": [
        "Sonic CD", "Lunar: The Silver Star", "Lunar 2: Eternal Blue", "Snatcher",
        "Popful Mail", "Shining Force CD", "Lords of Thunder", "Nightmare Circus" ,
        "Robo Aleste", "Axelay" , "Silpheed", "Sewer Shark", "Night Trap",
        "Double Switch", "Ground Zero Texas", "Corpse Killer", "Supreme Warrior",
        "Make My Video: Kriss Kross", "Wirehead", "Fahrenheit", "Dragon's Lair",
        "Space Ace", "Time Gal", "Road Avenger", "Cobra Command",
        "Sol-Feace", "Cobra" , "The Adventures of Willy Beamish", "Slam City with Scottie Pippen",
        "Switch", "Panic!", "Keio Flying Squadron", "Sylvan Tale" , "Battlecorps",
        "Final Fight CD", "The Amazing Spider-Man vs. The Kingpin", "Spider-Man vs. The Kingpin",
        "Batman Returns", "The Terminator", "RoboCop vs. The Terminator", "Jungle Strike",
        "Urban Strike", "Sonic the Hedgehog CD", "Ecco the Dolphin", "Ecco: The Tides of Time",
        "Eternal Champions: Challenge from the Dark Side", "Cosmic Fantasy Stories",
        "Vay", "Dark Wizard", "Third World War", "Dune", "The Misadventures of Flink",
        "Wonder Boy in Monster World" , "Monster World IV", "Golden Axe", "Altered Beast",
        "Columns III" , "Shining in the Darkness" , "Fatal Fury Special", "Art of Fighting",
        "Samurai Shodown", "World Heroes", "ASO II: Last Guardian" , "Lethal Enforcers",
        "Lethal Enforcers II: Gun Fighters", "Crime Patrol", "Mad Dog McCree",
        "Who Shot Johnny Rock?", "Bram Stoker's Dracula", "Mickey Mania: The Timeless Adventures of Mickey Mouse",
        "The Nutshack" , "Dungeons & Dragons: Warriors of the Eternal Sun" ,
    ],
    "pcecd": [
        "Akumajou Dracula X: Chi no Rondo", "Castlevania: Rondo of Blood", "Ys Book I & II",
        "Ys I & II", "Ys III: Wanderers from Ys", "Ys IV: The Dawn of Ys", "Snatcher",
        "Bomberman 93" , "Bomberman '94", "Detana!! TwinBee", "TwinBee", "Gradius II",
        "Salamander", "Nemesis II" , "R-Type", "R-Type Complete CD" , "Darius Plus" ,
        "Darius Alpha" , "Gauntlet", "Madou King Granzort" , "Tengai Makyou: Deden no Den",
        "Tengai Makyou II: Manji Maru", "Tengai Makyou: Ziria", "Tengai Makyou Zero" ,
        "Neutopia", "Neutopia II", "The Legend of Hero Tonma" ,
        "Military Madness", "Nectaris" , "Blazing Lazers", "Gunhed", "Dragon Spirit",
        "Dragon Saber", "Ordyne", "Valkyrie no Densetsu", "Legend of Valkyrie" ,
        "Wonder Boy III: Monster Lair", "Monster Lair", "Alien Crush", "Devil's Crush",
        "Devil Crash", "Jaseiken Necromancer", "Ninja Spirit", "Moto Roader",
        "Moto Roader II", "Final Match Tennis", "Power Tennis" , "Formation Soccer",
        "J-League" , "F1 Pilot" , "Air Zonk", "Super Air Zonk" , "Bonk's Adventure",
        "Bonk 3: Bonk's Big Adventure", "PC Genjin", "PC Genjin 2", "PC Genjin 3",
        "Takin' It to the Hoop" , "Dungeon Explorer", "Dungeon Explorer II",
        "Cadash", "The Legendary Axe", "The Legendary Axe II", "Victory Run",
        "Metal Stoker", "Download", "Download 2", "Psychic Storm", "Seirei Senshi Spriggan",
        "Spriggan Mark 2", "Mashin Eiyuuden Wataru" , "Ane Sangokushi" , "Flash Hiders",
        "Blue Breaker", "Fray" , "Emerald Dragon", "Megami Tensei", "Dungeon Master" ,
        "Shera, My Witch" , "Steam Hearts", "Valsfar" , "Super Darius", "Super Darius II",
        "Chou Aniki", "Aero Blasters", "Kisou Louga" , "Popful Mail" ,
    ],
}

# ---------------------------------------------------------------------------
# Region and disc tag conventions (mirrors redump.rs extract_region/extract_disc_info)
# ---------------------------------------------------------------------------
REGION_TAGS = {  # tag variant -> canonical label
    "USA": "USA", "U": "USA", "US": "USA", "NTSC": "USA", "NTSC-U": "USA", "North America": "USA",
    "Europe": "Europe", "E": "Europe", "EU": "Europe", "PAL": "Europe", "Euro": "Europe",
    "Japan": "Japan", "J": "Japan", "JPN": "Japan", "NTSC-J": "Japan",
    "World": "World", "W": "World",
}
DISC_WORDS = ["Disc", "Disk", "disc", "disk", "CD", "cd"]

EXTS = [".cue", ".bin", ".iso", ".img", ".gdi", ".chd", ".7z", ".zip", ""]
SCENE_JUNK = ["[!]", "[b]", "(Rev A)", "(Rev B)", "(v1.1)", "(v2.0)", "[SLUS-01234]",
              "[SCES-01234]", "(Track 1)", "(track01)", "(RIP)", "(PROPER)", "(REPACK)"]
PLATFORM_FOLDERS = {
    "psx": ["PSX", "psx", "Sony PlayStation", "playstation", "PS1", "PS"],
    "saturn": ["Saturn", "saturn", "Sega Saturn", "sega-saturn"],
    "dreamcast": ["Dreamcast", "dreamcast", "DC", "Sega Dreamcast"],
    "segacd": ["SegaCD", "segacd", "Sega CD", "MegaCD", "Mega CD"],
    "pcecd": ["PCECD", "pcecd", "PC Engine CD", "TurboGrafx-CD", "TurboDuo"],
}
NEGATIVES = [
    ("System Firmware 3.15 Update.iso", None),
    ("BIOS Pack Complete (2026).zip", None),
    ("RetroArch Cores Bundle v1.20.7z", None),
    ("Emulator Frontend Installer (Windows).exe", None),
    ("Save State Pack - All Games (USA).zip", None),
    ("Boxart Collection Pack 2026.7z", None),
]

CLASSIFY_TOOL = {
    "name": "classify_rom",
    "description": "Classify a retro disc-image filename into its target console platform, multi-disc status, disc number, and release region.",
    "parameters": {
        "type": "object",
        "properties": {
            "platform": {"type": "string", "enum": ["psx", "saturn", "dreamcast", "segacd", "pcecd"],
                          "description": "Target console: psx=Sony PlayStation, saturn=Sega Saturn, dreamcast=Sega Dreamcast, segacd=Sega CD / Mega CD, pcecd=PC Engine CD / TurboGrafx-CD"},
            "is_multidisc": {"type": "boolean",
                              "description": "True if this disc belongs to a multi-disc game release"},
            "disc_number": {"type": "string", "enum": ["single", "1", "2", "3", "4", "5", "6", "7", "8"],
                             "description": "Disc number within the set, or single for a standalone release"},
            "region": {"type": "string", "enum": ["USA", "Europe", "Japan", "World"],
                        "description": "Primary release region"},
        },
        "required": ["platform", "is_multidisc", "disc_number", "region"],
    },
}

def region_tag(canonical: str, rng) -> str:
    variants = [t for t, c in REGION_TAGS.items() if c == canonical]
    return rng.choice(variants)

def make_query(title: str, platform: str, rng) -> dict:
    """Build one messy filename + its ground-truth label."""
    # --- label decisions ---
    region = rng.choice(["USA", "USA", "Europe", "Japan", "World"])  # USA-weighted like real collections
    multi = rng.random() < 0.35
    disc_n = rng.randint(1, 4) if multi else None
    style = rng.random()

    parts = []
    # title mangling
    t = title
    if style < 0.20:
        t = t.lower()
    if rng.random() < 0.25:
        t = t.replace(" ", "_")
    elif rng.random() < 0.12:
        t = t.replace(" ", ".")
    if rng.random() < 0.10:
        t = t.replace(":", "").replace("'", "").replace("!", "")
    parts.append(t)

    # region tag (always present in some form; label derives from it)
    tag = region_tag(region, rng)
    wrap = rng.random()
    if wrap < 0.55:
        parts.append(f"({tag})")
    elif wrap < 0.75:
        parts.append(f"[{tag}]")
    else:
        parts.append(tag)

    # disc tag
    if multi:
        word = rng.choice(DISC_WORDS)
        fmt = rng.random()
        if fmt < 0.45:
            parts.append(f"({word} {disc_n})")
        elif fmt < 0.70:
            parts.append(f"{word} {disc_n}")
        elif fmt < 0.85:
            parts.append(f"{word.lower().replace(' ', '')}{disc_n}")
        else:
            parts.append(f"({word} {disc_n} of {disc_n + rng.randint(0, 2)})")

    sep = rng.choice([" ", "_", ".", " ", " ", "-"])
    name = sep.join(parts) if sep != "-" else "-".join(parts)

    if rng.random() < 0.30:
        name += " " + rng.choice(SCENE_JUNK)
    # .gdi is a Dreamcast-only descriptor; other platforms never produce one
    ext = rng.choice(EXTS if platform == "dreamcast" else [e for e in EXTS if e != ".gdi"])
    fname = name + ext

    # folder hint sometimes
    if rng.random() < 0.30:
        folder = rng.choice(PLATFORM_FOLDERS[platform])
        fname = folder + "/" + fname

    return {
        "query": fname,
        "answer": {
            "name": "classify_rom",
            "arguments": {
                "platform": platform,
                "is_multidisc": multi,
                "disc_number": str(disc_n) if multi else "single",
                "region": region,
            },
        },
    }

def split_key(title: str, platform: str) -> float:
    h = hashlib.sha256(f"{platform}|{title}".encode()).hexdigest()
    return int(h[:8], 16) / 0xFFFFFFFF

def main():
    out_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "data")
    os.makedirs(out_dir, exist_ok=True)
    train, val, eval_ = [], [], []

    for platform, titles in TITLES.items():
        seen = set()
        for title in titles:
            if title in seen:
                continue
            seen.add(title)
            k = split_key(title, platform)
            # multiple variants per title in train; 1 variant for val/eval titles
            if k < 0.75:
                for _ in range(4):
                    ex = make_query(title, platform, random)
                    train.append(ex)
            elif k < 0.875:
                val.append(make_query(title, platform, random))
            else:
                eval_.append(make_query(title, platform, random))

    # negatives: 4% of train, few in eval
    negs = [{"query": q, "answer": None} for q, _ in NEGATIVES]
    train.extend(negs * 3)
    eval_.extend(negs[:3])

    random.shuffle(train)

    def write(name, rows):
        with open(os.path.join(out_dir, name), "w", encoding="utf-8") as f:
            for r in rows:
                obj = {
                    "query": r["query"],
                    "tools": [CLASSIFY_TOOL],
                    "answers": [r["answer"]] if r["answer"] else [],
                }
                f.write(json.dumps(obj, ensure_ascii=False) + "\n")
        print(f"{name}: {len(rows)}")

    write("train.jsonl", train)
    write("validation.jsonl", val)
    write("test.jsonl", eval_)

    # quick label sanity
    from collections import Counter
    c = Counter(r["answer"]["arguments"]["platform"] if r["answer"] else "NEG" for r in train + val + eval_)
    print("platform distribution:", dict(c))
    c2 = Counter(r["answer"]["arguments"]["region"] if r["answer"] else "NEG" for r in train + val + eval_)
    print("region distribution:", dict(c2))
    print("\nsamples:")
    for r in random.sample(train, 5):
        print(" ", json.dumps(r, ensure_ascii=False)[:160])

if __name__ == "__main__":
    main()
