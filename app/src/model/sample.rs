//! The sample library the demo and the screenshots use.

use super::*;

pub(crate) fn chapter(_id: u64, title: &str, body: &str, summary: &str, source: &str) -> Chapter {
    Chapter {
        key: String::new(),
        title: title.to_string(),
        body: body.trim().to_string(),
        summary: summary.trim().to_string(),
        summary_source: source.to_string(),
        summary_body_len: body.trim().chars().count(),
        summary_body_hash: content_hash(body.trim()),
        chat: Vec::new(),
        chat_summary: String::new(),
        steering: String::new(),
        draft: None,
    }
}

pub(crate) fn lore(
    _id: u64,
    name: &str,
    category: &str,
    aliases: &str,
    triggers: &str,
    description: &str,
    pinned: bool,
) -> LoreEntry {
    LoreEntry {
        key: String::new(),
        name: name.to_string(),
        category: category.to_string(),
        aliases: aliases.to_string(),
        triggers: triggers.to_string(),
        description: description.to_string(),
        pinned,
        invented: false,
    }
}

pub(crate) fn sample_lantern_coast() -> Story {
    Story {
        key: String::new(),
        created_at: 0,
        next_chapter: 1,
        next_lore: 1,
        title: "The Lantern Coast".to_string(),
        premise: "A cartographer returns to the drowned harbour of her childhood and finds her mother's last map pointing to an island that appears on no tide.".to_string(),
        chapters: vec![
            chapter(
                11,
                "The Tide Map",
                "The sea went out on a Tuesday and did not come back.

Mira Vale stood at the end of the Brinehouse pier with her suitcase still in her hand. Twelve years had passed since she had last seen the harbour, and in that time she had convinced herself that memory had exaggerated it. It had not. The town still leaned into the wind like a conversation, its roofs green with salt, its windows full of the grey, patient light of the northern coast. What was wrong was the water. Where there should have been thirty feet of tide, there was mud: ribs of rock, old chains, the black bones of a boat that had gone down before she was born.

\"You picked a fine evening for it,\" said Wren Okafor, coming down the pier with a lantern in each hand. Wren had been old when Mira was a child. Now they looked like something the sea had worn smooth. \"The Ninth Tide,\" they said, before Mira could ask. \"Happens once in a generation, if the stories are true. The bay empties and shows you what it's been keeping.\"

Wren set one lantern on the boards between them. In its light Mira saw the map Wren was carrying: her mother's map, the one that had hung in the map room at home, the one that had been missing from the estate for twelve years. It was stained the colour of weak tea and covered in small, precise marks.

\"She left it with me,\" Wren said. \"Told me to give it to you when the water went out.\"

Mira unrolled it. The coastline was her mother's hand, unmistakable: every inlet and shingle drawn with the patience of someone who believed the land deserved accuracy. But there was an island in the bay that Mira did not recognise. It sat north of the harbour, ringed by a dotted line, and beside it her mother had written four words in pencil: not on any tide.

She looked up. Out in the drained bay, a line of pale stones caught the lantern light. Saltglass. Her mother used to collect it from the beaches, the greenish shards that the sea polished like bottle bottoms. The stones ran in a straight line across the mud, out toward the place where the island should have been.

\"Don't,\" Wren said, but they did not move to stop her.

Mira took off her shoes. The mud was cold enough to ache. She followed the saltglass, one stone and then the next, and with every step the town behind her seemed to grow smaller, as if she were walking out of a story rather than into one.

Halfway across, she heard a bell.

It came from under the water. It came from under the mud. It came from a long way down, in a room full of drowned furniture, and it was ringing the way a bell rings when someone is holding it in their hand.

She turned back. The pier was a black line against the lanterns, and Wren was a smaller black line on the pier. Beyond them, in the window of her mother's house, a light was burning.

The house had been empty for twelve years.

Mira began to run.",
                "Mira returns to Brinehouse after twelve years and finds the bay drained by a rare Ninth Tide. Wren Okafor gives her her mother's tide map, which marks an island \"not on any tide\" out in the exposed bay. Following a line of saltglass across the mud, Mira hears a bell beneath the seabed and sees a light burning in her long-empty childhood home.",
                "ai",
            ),
            chapter(
                12,
                "A House With No Door",
                "Her mother's house had no door.

That was not quite true. The hinges were there, two black curves screwed into the frame, and the brass number 9 was still crooked above them. But where the door should have been, someone had built a wall of grey stones, fitted so tightly that Mira could not see the mortar. The stones were wet. The whole street was dry.

Mira put her hand against them. They were warm.

\"It was like that when I came back,\" said a voice behind her.

She turned. A boy stood at the gate, perhaps twelve, wearing a coat three sizes too large. He had the careful stillness of a child who has learned not to be noticed. \"You're the daughter,\" he said. \"The one who draws maps.\"

\"I'm a cartographer,\" Mira said. \"There's a difference.\"

\"Is there?\"

She looked back at the wall. \"Who did this?\"

The boy shrugged. \"It was here before the tide went out. My grandmother says the house has been waiting for someone. She says houses do that.\" He held out a key. It was green with salt, and much too large for any door Mira had ever seen. \"She said to give you this when you came back. She said you'd know which one it opens.\"

The key was cold. Mira turned it over. On the bow, in letters nearly worn away, was a name she did not know: E. VALE.

Her mother's name had been Ada.

Inside the house — because the key did open something, though not the wall; it opened the old garden gate, which had been locked since Mira was nine — the air smelled of paper and rain. The map room was at the back, and it was exactly as she remembered it: the long table, the brass lamp, the shelves of rolled charts in their wooden slots. Someone had been here. The lamp was lit. A cup of tea sat beside it, still steaming.

On the table lay a notebook open to a page of tide tables. Her mother's handwriting filled the margins. The bay is not empty, one note read. It is holding its breath.

Beneath it, in different ink: Ask the bell.

Mira sat down in her mother's chair. Outside, the sea began to come back. It came all at once, the way a held breath comes out, and the house settled around her with a sound like a sigh.",
                "Mira finds her mother's house sealed with a wall of warm stones and receives an oversized salt-green key from a watchful neighbourhood boy. The key opens the garden gate, not the wall, and inside she discovers the map room lit and a fresh cup of tea. Her mother's notebook says the bay is holding its breath and adds, in another hand, \"Ask the bell.\" The tide returns around the house as Mira sits in her mother's chair.",
                "ai",
            ),
            chapter(
                13,
                "Salt in the Wiring",
                "The bell was still ringing when Mira woke.

She had fallen asleep at the map table with her cheek on the tide tables. The lamp had burned down to a blue bead. Outside, the harbour was full again, black and patient, and the town lights moved on it in long, broken lines.

The notebook was open to a new page.

She was certain it had not been, but there it was: a drawing of the house, and beneath it, in her mother's hand, a sentence that had not been there the night before.

The island is not lost. It is waiting to be remembered.",
                "",
                "",
            ),
        ],
        lore: vec![
            lore(21, "Mira Vale", "Character", "the cartographer, Mira", "the daughter, the map table", "A cartographer in her thirties who left Brinehouse at eighteen and has spent her adult life drawing coastlines she never visits. Precise, stubborn, and unsettled by anything she cannot measure.", true),
            lore(22, "Wren Okafor", "Character", "Wren, the lantern keeper", "the old friend, a lantern in each hand", "An old family friend who keeps the harbour lights and remembers more about Ada Vale than they are willing to say. Speaks in short sentences and never carries only one lantern.", true),
            lore(23, "Brinehouse", "Place", "the harbour, the town", "the pier, the houses", "A small northern harbour town built into a wedge of black rock. Its houses lean into the wind; its streets smell of salt, rope and woodsmoke. The sea has been taking pieces of it for a hundred years.", true),
            lore(24, "The Ninth Tide", "Lore", "the low tide, the emptying", "the drained bay, the seabed", "A tide that arrives once in a generation and pulls the whole bay dry, exposing the seabed for one night. Locals disagree about what it is for. Everyone agrees you do not walk out on it.", true),
            lore(25, "Saltglass", "Item", "sea glass, the pale stones", "the green shards, the lantern light", "Greenish shards polished by the sea, found on the beaches around Brinehouse. Ada collected them in jars. In the drained bay they form a straight line no storm has ever moved.", false),
            lore(26, "The Lantern Coast", "Place", "the coast", "the northern coast, the cliffs", "The long stretch of cliffs and harbour towns where the story takes place. Sailors say the coast keeps a light for every ship it has taken.", false),
            lore(27, "Ada Vale", "Character", "Mira's mother, E. Vale", "her mother, the mapmaker", "Mira's mother, a mapmaker who disappeared twelve years ago. Her notes are careful, her pencil marks exact, and her last map shows an island that should not exist.", true),
            lore(28, "Hollow Lanterns", "Lore", "the hollow lights", "no flame, a light burning", "Lanterns found along the coast with no flame inside and no maker's mark. They are said to burn only when someone is being remembered.", false),
        ],
        chat: Vec::new(),
        open_chapter: 1,
    }
}

pub(crate) fn sample_orchard() -> Story {
    Story {
        key: String::new(),
        created_at: 0,
        next_chapter: 1,
        next_lore: 1,
        title: "The Orchard at Night".to_string(),
        premise: "A night gardener tends trees that grow memories instead of fruit — until one tree begins to grow her own.".to_string(),
        chapters: vec![chapter(
            31,
            "First Frost",
            "The orchard only fruited after midnight.

Ivy had learned that the hard way, on her first week as keeper: she had gone out at noon with a basket and come back with nothing but a sunburn and the feeling of having been laughed at by trees. Now she walked the rows at one in the morning, lantern low, and listened for the small glassy sound of memories ripening.

The pears were ready. They hung from the branches like little moons, each one holding a scene from someone's life: a wedding, a kitchen, a dog asleep in a patch of sun. Ivy picked them with care and set them in the cart, where they chimed against one another. By morning they would be jam, and by winter they would be sold in the village as something to spread on toast when you needed to remember your grandmother's hands.

At the far end of the orchard, where the oldest tree stood, something was different.

There was one pear on it. It was not silver, like the others. It was red, and it was warm, and when Ivy held up her lantern she saw herself inside it: older, greyer, standing in this same orchard with a lantern in her hand.",
            "",
            "",
        )],
        lore: vec![
            lore(
                41,
                "Ivy Marchetti",
                "Character",
                "Ivy, the keeper",
                "the night gardener, the cart",
                "The orchard's night keeper. Practical, tired, and secretly proud of a job nobody else wanted. She has never tasted a memory fruit.",
                true,
            ),
            lore(
                42,
                "The Night Orchard",
                "Place",
                "the orchard",
                "the rows, the oldest tree",
                "An old orchard on the hill above the village. Its trees fruit only between midnight and dawn, and only for the person who tends them.",
                true,
            ),
            lore(
                43,
                "Memory Fruit",
                "Lore",
                "the pears, silver pears",
                "the jam, a memory ripening",
                "Pears that hold a single remembered moment. Eaten fresh they show you the memory; cooked into jam they give you only its feeling.",
                false,
            ),
            lore(
                44,
                "The Keeper's Rules",
                "Lore",
                "the rules",
                "before midnight, the orchard gate",
                "Never pick before midnight. Never eat what you pick. Never ask whose memory it was. The rules are written on the inside of the orchard gate, in a hand that changes every year.",
                true,
            ),
        ],
        chat: Vec::new(),
        open_chapter: 0,
    }
}
