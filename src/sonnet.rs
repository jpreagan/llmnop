use once_cell::sync::Lazy;
use rand::seq::SliceRandom;
use rand::thread_rng;

static SONNET_LINES: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        "Shall I compare thee to a summer's day?",
        "Thou art more lovely and more temperate:",
        "Rough winds do shake the darling buds of May,",
        "And summer's lease hath all too short a date:",
        "Sometime too hot the eye of heaven shines,",
        "And often is his gold complexion dimm'd;",
        "And every fair from fair sometime declines,",
        "By chance or nature's changing course untrimm'd;",
        "But thy eternal summer shall not fade",
        "Nor lose possession of that fair thou owest;",
        "Nor shall Death brag thou wander'st in his shade,",
        "When in eternal lines to time thou growest:",
        "So long as men can breathe or eyes can see,",
        "So long lives this and this gives life to thee.",
        "Then let not winter's ragged hand deface",
        "In thee thy summer, ere thou be distill'd:",
        "Make sweet some vial; treasure thou some place",
        "With beauty's treasure, ere it be self-kill'd.",
        "That use is not forbidden usury,",
        "Which happies those that pay the willing loan;",
        "That's for thyself to breed another thee,",
        "Or ten times happier, be it ten for one;",
        "Ten times thyself were happier than thou art,",
        "If ten of thine ten times refigured thee:",
        "Then what could death do, if thou shouldst depart,",
        "Leaving thee living in posterity?",
        "Be not self-will'd, for thou art much too fair",
        "To be death's conquest and make worms thine heir.",
        "Where art thou, Muse, that thou forget'st so long",
        "To speak of that which gives thee all thy might?",
        "Spend'st thou thy fury on some worthless song,",
        "Darkening thy power to lend base subjects light?",
        "Return, forgetful Muse, and straight redeem",
        "In gentle numbers time so idly spent;",
        "Sing to the ear that doth thy lays esteem",
        "And gives thy pen both skill and argument.",
        "Rise, resty Muse, my love's sweet face survey,",
        "If Time have any wrinkle graven there;",
        "If any, be a satire to decay,",
        "And make Time's spoils despised every where.",
        "Give my love fame faster than Time wastes life;",
        "So thou prevent'st his scythe and crooked knife.",
        "My glass shall not persuade me I am old,",
        "So long as youth and thou are of one date;",
        "But when in thee time's furrows I behold,",
        "Then look I death my days should expiate.",
        "For all that beauty that doth cover thee",
        "Is but the seemly raiment of my heart,",
        "Which in thy breast doth live, as thine in me:",
        "How can I then be elder than thou art?",
        "O, therefore, love, be of thyself so wary",
        "As I, not for myself, but for thee will;",
        "Bearing thy heart, which I will keep so chary",
        "As tender nurse her babe from faring ill.",
        "Presume not on thy heart when mine is slain;",
        "Thou gavest me thine, not to give back again.",
        "So am I as the rich, whose blessed key",
        "Can bring him to his sweet up-locked treasure,",
        "The which he will not every hour survey,",
        "For blunting the fine point of seldom pleasure.",
        "Therefore are feasts so solemn and so rare,",
        "Since, seldom coming, in the long year set,",
        "Like stones of worth they thinly placed are,",
        "Or captain jewels in the carcanet.",
        "So is the time that keeps you as my chest,",
        "Or as the wardrobe which the robe doth hide,",
        "To make some special instant special blest,",
        "By new unfolding his imprison'd pride.",
        "Blessed are you, whose worthiness gives scope,",
        "Being had, to triumph, being lack'd, to hope.",
        "If there be nothing new, but that which is",
        "Hath been before, how are our brains beguiled,",
        "Which, labouring for invention, bear amiss",
        "The second burden of a former child!",
        "O, that record could with a backward look,",
        "Even of five hundred courses of the sun,",
        "Show me your image in some antique book,",
        "Since mind at first in character was done!",
        "That I might see what the old world could say",
        "To this composed wonder of your frame;",
        "Whether we are mended, or whether better they,",
        "Or whether revolution be the same.",
        "O, sure I am, the wits of former days",
        "To subjects worse have given admiring praise.",
    ]
});

/// Returns the shuffled lines from the Shakespeare sonnet.
///
/// # Returns
///
/// * `Vec<&'static str>` - A vector of shuffled sonnet lines
pub fn get_shuffled_sonnet_lines() -> Vec<&'static str> {
    let mut rng = thread_rng();
    let mut lines = SONNET_LINES.clone();
    lines.shuffle(&mut rng);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_shuffled_sonnet_lines_length() {
        let shuffled_lines = get_shuffled_sonnet_lines();
        assert_eq!(shuffled_lines.len(), SONNET_LINES.len());
    }

    #[test]
    fn test_get_shuffled_sonnet_lines_content() {
        let shuffled_lines = get_shuffled_sonnet_lines();
        for line in shuffled_lines.iter() {
            assert!(SONNET_LINES.contains(line));
        }
    }
}
