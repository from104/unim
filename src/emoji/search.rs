//! 이모지 입력 모듈
//!
//! Windows 11 emoji panel과 동등한 범위(Unicode 15.0 fully-qualified, ~1900개)의
//! 이모지를 9개 메이저 카테고리로 제공합니다. 한국어/영어 키워드 검색을 모두
//! 지원합니다. 데이터는 `tools/gen_emoji.py`로 Unicode `emoji-test.txt`에서
//! 생성되어 `emoji_data.rs`에 정적으로 박혀 있습니다.
//!
//! 기존 특수문자 팝업 인프라(`ShowSpecial` 시그널)와 동일한 격자 UI를 재활용하기
//! 위해, 빈 키워드 검색은 인기 이모지를, 카테고리 ID 검색은 해당 카테고리 전체를
//! 반환합니다.

#[cfg(test)]
use super::data::TOTAL_EMOJIS;
use super::data::{CATEGORIES, EMOJIS};

/// 이모지 항목 (레거시 — 외부 호환용)
///
/// 새 코드는 `EmojiDatum`을 직접 사용하세요.
pub struct EmojiEntry {
    /// 키워드 (검색용, 한글)
    pub keyword: &'static str,
    /// 이모지 문자들
    pub emojis: &'static [char],
}

/// 인기 이모지 (기본 표시용 — 빈 키워드 검색 결과)
const POPULAR_EMOJIS: &[&str] = &[
    "😀", "😂", "🥲", "😍", "🥰", "😊", "😎", "🤔", "😅", "😭", "🙏", "👍", "👎", "❤️", "🔥", "✨",
    "🎉", "💯", "👀", "🤣", "😱", "😤", "🥺", "😢", "💪", "🙌", "👏", "🤝", "✅", "❌", "⭐", "💡",
    "📌", "🎵", "🎶", "☕", "🍕", "🍺", "🚀", "💻", "📱", "⏰", "🔔", "📧", "🗓️", "📎", "✏️", "📝",
    "🔑", "🏠", "🚗", "✈️", "🌍", "🌙", "☀️", "🌧️", "❄️", "🌸", "🍀", "🌈", "🐱", "🐶", "🦊", "🐻",
    "🐼", "🐰", "🦁", "🐯", "🐸", "🦄", "💎", "🎁", "🎂", "🏆", "🥇", "🎯", "♻️", "💤", "💬", "🔗",
];

/// 한국어 키워드 → 영어 검색 토큰 매핑.
///
/// `search_emoji`가 한글 키워드를 받으면 우선 이 표에서 찾고, 매핑된 영어 토큰을
/// `en_name`/`subgroup`과 substring 매치합니다. CLDR `ko.xml`이 시스템에 없어
/// 자동 추출이 불가하므로 자주 쓰이는 표현 위주로 큐레이션했습니다.
struct KoKeyword {
    ko: &'static str,
    /// 영어 검색 토큰 — `en_name` 또는 `subgroup`에 substring 매치되면 hit.
    en_tokens: &'static [&'static str],
}

const KO_KEYWORDS: &[KoKeyword] = &[
    // 표정·감정
    KoKeyword {
        ko: "웃음",
        en_tokens: &["grinning", "smiling", "smile", "laugh", "joy"],
    },
    KoKeyword {
        ko: "웃는",
        en_tokens: &["grinning", "smiling", "smile"],
    },
    KoKeyword {
        ko: "행복",
        en_tokens: &["grinning", "smiling", "joy"],
    },
    KoKeyword {
        ko: "사랑",
        en_tokens: &["heart", "love", "kiss", "smiling face with heart"],
    },
    KoKeyword {
        ko: "심장",
        en_tokens: &["heart"],
    },
    KoKeyword {
        ko: "하트",
        en_tokens: &["heart"],
    },
    KoKeyword {
        ko: "키스",
        en_tokens: &["kiss"],
    },
    KoKeyword {
        ko: "윙크",
        en_tokens: &["winking"],
    },
    KoKeyword {
        ko: "슬픔",
        en_tokens: &[
            "sad",
            "crying",
            "cry",
            "loudly crying",
            "pensive",
            "frowning",
        ],
    },
    KoKeyword {
        ko: "울음",
        en_tokens: &["crying", "cry"],
    },
    KoKeyword {
        ko: "눈물",
        en_tokens: &["crying", "tear"],
    },
    KoKeyword {
        ko: "화남",
        en_tokens: &["angry", "rage", "pouting", "anger"],
    },
    KoKeyword {
        ko: "분노",
        en_tokens: &["angry", "rage", "pouting"],
    },
    KoKeyword {
        ko: "놀람",
        en_tokens: &["astonished", "surprised", "shocked", "screaming"],
    },
    KoKeyword {
        ko: "충격",
        en_tokens: &["shocked", "screaming", "astonished"],
    },
    KoKeyword {
        ko: "지침",
        en_tokens: &["tired", "weary", "sleepy"],
    },
    KoKeyword {
        ko: "졸림",
        en_tokens: &["sleepy", "sleeping", "yawning"],
    },
    KoKeyword {
        ko: "피곤",
        en_tokens: &["tired", "weary", "exhausted"],
    },
    KoKeyword {
        ko: "병",
        en_tokens: &[
            "face with thermometer",
            "ill",
            "nauseated",
            "sneezing",
            "mask",
        ],
    },
    KoKeyword {
        ko: "아픔",
        en_tokens: &["face with thermometer", "ill", "nauseated"],
    },
    KoKeyword {
        ko: "생각",
        en_tokens: &["thinking"],
    },
    KoKeyword {
        ko: "쿨",
        en_tokens: &["sunglasses", "cool"],
    },
    KoKeyword {
        ko: "혀",
        en_tokens: &["tongue"],
    },
    KoKeyword {
        ko: "악마",
        en_tokens: &["devil", "imp", "demon"],
    },
    KoKeyword {
        ko: "유령",
        en_tokens: &["ghost"],
    },
    KoKeyword {
        ko: "광대",
        en_tokens: &["clown"],
    },
    KoKeyword {
        ko: "외계인",
        en_tokens: &["alien"],
    },
    KoKeyword {
        ko: "로봇",
        en_tokens: &["robot"],
    },
    KoKeyword {
        ko: "고양이",
        en_tokens: &["cat"],
    },
    KoKeyword {
        ko: "원숭이",
        en_tokens: &["monkey", "see-no-evil"],
    },
    // 사람·신체
    KoKeyword {
        ko: "손",
        en_tokens: &[
            "hand",
            "thumbs",
            "clapping",
            "raising hands",
            "ok hand",
            "waving",
            "raised hand",
        ],
    },
    KoKeyword {
        ko: "손가락",
        en_tokens: &["finger", "pointing", "ok hand", "victory"],
    },
    KoKeyword {
        ko: "박수",
        en_tokens: &["clapping"],
    },
    KoKeyword {
        ko: "엄지",
        en_tokens: &["thumbs"],
    },
    KoKeyword {
        ko: "주먹",
        en_tokens: &["fist"],
    },
    KoKeyword {
        ko: "악수",
        en_tokens: &["handshake"],
    },
    KoKeyword {
        ko: "기도",
        en_tokens: &["folded hands"],
    },
    KoKeyword {
        ko: "근육",
        en_tokens: &["flexed biceps", "muscle"],
    },
    KoKeyword {
        ko: "사람",
        en_tokens: &["person", "man", "woman", "people"],
    },
    KoKeyword {
        ko: "남자",
        en_tokens: &["man "],
    },
    KoKeyword {
        ko: "여자",
        en_tokens: &["woman"],
    },
    KoKeyword {
        ko: "아기",
        en_tokens: &["baby"],
    },
    KoKeyword {
        ko: "아이",
        en_tokens: &["child", "boy", "girl"],
    },
    KoKeyword {
        ko: "노인",
        en_tokens: &["older person", "old man", "old woman"],
    },
    KoKeyword {
        ko: "가족",
        en_tokens: &["family"],
    },
    KoKeyword {
        ko: "커플",
        en_tokens: &["couple"],
    },
    KoKeyword {
        ko: "결혼",
        en_tokens: &["bride", "wedding", "couple with heart"],
    },
    KoKeyword {
        ko: "임신",
        en_tokens: &["pregnant"],
    },
    KoKeyword {
        ko: "수유",
        en_tokens: &["breast-feeding"],
    },
    KoKeyword {
        ko: "직업",
        en_tokens: &[
            "worker",
            "doctor",
            "teacher",
            "judge",
            "farmer",
            "cook",
            "police",
            "firefighter",
        ],
    },
    KoKeyword {
        ko: "경찰",
        en_tokens: &["police"],
    },
    KoKeyword {
        ko: "의사",
        en_tokens: &["health worker", "doctor"],
    },
    KoKeyword {
        ko: "선생",
        en_tokens: &["teacher"],
    },
    KoKeyword {
        ko: "산타",
        en_tokens: &["santa"],
    },
    KoKeyword {
        ko: "왕",
        en_tokens: &["prince", "king crown"],
    },
    KoKeyword {
        ko: "공주",
        en_tokens: &["princess"],
    },
    KoKeyword {
        ko: "마법사",
        en_tokens: &["mage"],
    },
    KoKeyword {
        ko: "요정",
        en_tokens: &["fairy", "elf"],
    },
    KoKeyword {
        ko: "달리기",
        en_tokens: &["running"],
    },
    KoKeyword {
        ko: "걷기",
        en_tokens: &["walking"],
    },
    KoKeyword {
        ko: "춤",
        en_tokens: &["dancing", "dancer"],
    },
    KoKeyword {
        ko: "수영",
        en_tokens: &["swimming"],
    },
    // 동물·자연
    KoKeyword {
        ko: "동물",
        en_tokens: &[
            "dog", "cat", "horse", "cow", "pig", "rabbit", "bear", "fox", "wolf",
        ],
    },
    KoKeyword {
        ko: "강아지",
        en_tokens: &["dog"],
    },
    KoKeyword {
        ko: "개",
        en_tokens: &["dog"],
    },
    KoKeyword {
        ko: "곰",
        en_tokens: &["bear"],
    },
    KoKeyword {
        ko: "토끼",
        en_tokens: &["rabbit"],
    },
    KoKeyword {
        ko: "여우",
        en_tokens: &["fox"],
    },
    KoKeyword {
        ko: "사자",
        en_tokens: &["lion"],
    },
    KoKeyword {
        ko: "호랑이",
        en_tokens: &["tiger"],
    },
    KoKeyword {
        ko: "말",
        en_tokens: &["horse"],
    },
    KoKeyword {
        ko: "소",
        en_tokens: &["cow", "ox", "bull", "water buffalo"],
    },
    KoKeyword {
        ko: "돼지",
        en_tokens: &["pig", "boar"],
    },
    KoKeyword {
        ko: "쥐",
        en_tokens: &["mouse", "rat"],
    },
    KoKeyword {
        ko: "새",
        en_tokens: &["bird", "eagle", "duck", "owl", "swan", "rooster", "chicken"],
    },
    KoKeyword {
        ko: "물고기",
        en_tokens: &["fish", "shark", "blowfish"],
    },
    KoKeyword {
        ko: "바다",
        en_tokens: &["whale", "octopus", "dolphin", "shark", "shell"],
    },
    KoKeyword {
        ko: "벌레",
        en_tokens: &["bug", "ant", "beetle", "spider", "butterfly", "honeybee"],
    },
    KoKeyword {
        ko: "공룡",
        en_tokens: &["t-rex", "sauropod", "dinosaur"],
    },
    KoKeyword {
        ko: "유니콘",
        en_tokens: &["unicorn"],
    },
    KoKeyword {
        ko: "꽃",
        en_tokens: &[
            "rose",
            "tulip",
            "blossom",
            "sunflower",
            "hibiscus",
            "bouquet",
        ],
    },
    KoKeyword {
        ko: "나무",
        en_tokens: &["tree"],
    },
    KoKeyword {
        ko: "잎",
        en_tokens: &["leaf", "leaves", "herb", "shamrock", "four leaf clover"],
    },
    KoKeyword {
        ko: "식물",
        en_tokens: &["plant", "potted", "cactus", "tree", "leaf", "herb"],
    },
    KoKeyword {
        ko: "선인장",
        en_tokens: &["cactus"],
    },
    KoKeyword {
        ko: "버섯",
        en_tokens: &["mushroom"],
    },
    KoKeyword {
        ko: "날씨",
        en_tokens: &[
            "sun",
            "cloud",
            "rain",
            "snow",
            "lightning",
            "thunder",
            "rainbow",
            "tornado",
        ],
    },
    KoKeyword {
        ko: "비",
        en_tokens: &["rain"],
    },
    KoKeyword {
        ko: "눈",
        en_tokens: &["snow", "snowflake"],
    },
    KoKeyword {
        ko: "구름",
        en_tokens: &["cloud"],
    },
    KoKeyword {
        ko: "해",
        en_tokens: &["sun"],
    },
    KoKeyword {
        ko: "달",
        en_tokens: &["moon"],
    },
    KoKeyword {
        ko: "별",
        en_tokens: &["star"],
    },
    KoKeyword {
        ko: "무지개",
        en_tokens: &["rainbow"],
    },
    KoKeyword {
        ko: "지구",
        en_tokens: &["earth", "globe"],
    },
    KoKeyword {
        ko: "불",
        en_tokens: &["fire"],
    },
    // 음식·음료
    KoKeyword {
        ko: "음식",
        en_tokens: &[
            "pizza", "burger", "rice", "ramen", "sushi", "taco", "noodle", "bread", "sandwich",
        ],
    },
    KoKeyword {
        ko: "과일",
        en_tokens: &[
            "apple",
            "banana",
            "grape",
            "watermelon",
            "lemon",
            "orange",
            "peach",
            "strawberry",
        ],
    },
    KoKeyword {
        ko: "사과",
        en_tokens: &["apple"],
    },
    KoKeyword {
        ko: "바나나",
        en_tokens: &["banana"],
    },
    KoKeyword {
        ko: "포도",
        en_tokens: &["grape"],
    },
    KoKeyword {
        ko: "야채",
        en_tokens: &["carrot", "potato", "corn", "onion", "broccoli", "cucumber"],
    },
    KoKeyword {
        ko: "채소",
        en_tokens: &["carrot", "potato", "corn", "onion", "broccoli"],
    },
    KoKeyword {
        ko: "고기",
        en_tokens: &["meat", "poultry", "bacon", "steak"],
    },
    KoKeyword {
        ko: "빵",
        en_tokens: &["bread", "bagel", "croissant"],
    },
    KoKeyword {
        ko: "치즈",
        en_tokens: &["cheese"],
    },
    KoKeyword {
        ko: "피자",
        en_tokens: &["pizza"],
    },
    KoKeyword {
        ko: "햄버거",
        en_tokens: &["hamburger"],
    },
    KoKeyword {
        ko: "초밥",
        en_tokens: &["sushi"],
    },
    KoKeyword {
        ko: "국수",
        en_tokens: &["noodle", "ramen"],
    },
    KoKeyword {
        ko: "디저트",
        en_tokens: &[
            "cake",
            "ice cream",
            "doughnut",
            "cookie",
            "candy",
            "lollipop",
            "chocolate",
            "pie",
        ],
    },
    KoKeyword {
        ko: "케이크",
        en_tokens: &["cake"],
    },
    KoKeyword {
        ko: "쿠키",
        en_tokens: &["cookie"],
    },
    KoKeyword {
        ko: "음료",
        en_tokens: &[
            "coffee", "tea", "beer", "wine", "cocktail", "drink", "cup", "glass", "bottle",
        ],
    },
    KoKeyword {
        ko: "커피",
        en_tokens: &["coffee", "hot beverage"],
    },
    KoKeyword {
        ko: "차",
        en_tokens: &["teacup"],
    },
    KoKeyword {
        ko: "맥주",
        en_tokens: &["beer"],
    },
    KoKeyword {
        ko: "와인",
        en_tokens: &["wine"],
    },
    KoKeyword {
        ko: "술",
        en_tokens: &["beer", "wine", "cocktail", "tumbler", "sake"],
    },
    // 여행·장소
    KoKeyword {
        ko: "교통",
        en_tokens: &[
            "car",
            "bus",
            "train",
            "automobile",
            "taxi",
            "truck",
            "motorcycle",
            "scooter",
        ],
    },
    KoKeyword {
        ko: "자동차",
        en_tokens: &["automobile", "car", "taxi"],
    },
    KoKeyword {
        ko: "버스",
        en_tokens: &["bus"],
    },
    KoKeyword {
        ko: "기차",
        en_tokens: &["train", "locomotive"],
    },
    KoKeyword {
        ko: "비행기",
        en_tokens: &["airplane", "aircraft"],
    },
    KoKeyword {
        ko: "배",
        en_tokens: &["ship", "boat", "ferry", "sailboat", "canoe"],
    },
    KoKeyword {
        ko: "자전거",
        en_tokens: &["bicycle"],
    },
    KoKeyword {
        ko: "오토바이",
        en_tokens: &["motorcycle", "motor scooter"],
    },
    KoKeyword {
        ko: "로켓",
        en_tokens: &["rocket"],
    },
    KoKeyword {
        ko: "헬리콥터",
        en_tokens: &["helicopter"],
    },
    KoKeyword {
        ko: "여행",
        en_tokens: &["airplane", "luggage", "passport", "ticket"],
    },
    KoKeyword {
        ko: "건물",
        en_tokens: &[
            "building", "house", "office", "factory", "hotel", "school", "hospital",
        ],
    },
    KoKeyword {
        ko: "집",
        en_tokens: &["house", "home"],
    },
    KoKeyword {
        ko: "산",
        en_tokens: &["mountain", "volcano"],
    },
    KoKeyword {
        ko: "바닷가",
        en_tokens: &["beach", "shore", "umbrella", "sunrise"],
    },
    KoKeyword {
        ko: "사막",
        en_tokens: &["desert"],
    },
    KoKeyword {
        ko: "다리",
        en_tokens: &["bridge"],
    },
    KoKeyword {
        ko: "분수",
        en_tokens: &["fountain"],
    },
    KoKeyword {
        ko: "지도",
        en_tokens: &["map", "globe"],
    },
    KoKeyword {
        ko: "시계",
        en_tokens: &["clock", "watch", "alarm", "hourglass", "stopwatch", "timer"],
    },
    // 활동
    KoKeyword {
        ko: "스포츠",
        en_tokens: &[
            "soccer",
            "basketball",
            "baseball",
            "football",
            "tennis",
            "ball",
            "golf",
            "skateboard",
        ],
    },
    KoKeyword {
        ko: "축구",
        en_tokens: &["soccer"],
    },
    KoKeyword {
        ko: "농구",
        en_tokens: &["basketball"],
    },
    KoKeyword {
        ko: "야구",
        en_tokens: &["baseball"],
    },
    KoKeyword {
        ko: "테니스",
        en_tokens: &["tennis"],
    },
    KoKeyword {
        ko: "골프",
        en_tokens: &["golf"],
    },
    KoKeyword {
        ko: "메달",
        en_tokens: &["medal"],
    },
    KoKeyword {
        ko: "트로피",
        en_tokens: &["trophy"],
    },
    KoKeyword {
        ko: "축하",
        en_tokens: &["party", "tada", "confetti", "balloon", "fireworks"],
    },
    KoKeyword {
        ko: "파티",
        en_tokens: &["party", "tada", "confetti", "balloon"],
    },
    KoKeyword {
        ko: "선물",
        en_tokens: &["gift", "wrapped"],
    },
    KoKeyword {
        ko: "게임",
        en_tokens: &[
            "video game",
            "joystick",
            "chess",
            "dart",
            "puzzle",
            "dice",
            "playing card",
        ],
    },
    KoKeyword {
        ko: "예술",
        en_tokens: &["palette", "art", "performing arts"],
    },
    KoKeyword {
        ko: "음악",
        en_tokens: &[
            "musical",
            "music",
            "guitar",
            "violin",
            "saxophone",
            "trumpet",
            "drum",
            "piano",
            "note",
        ],
    },
    // 사물
    KoKeyword {
        ko: "옷",
        en_tokens: &[
            "clothes", "shirt", "jeans", "dress", "kimono", "sari", "swimsuit", "scarf", "gloves",
            "coat", "sock",
        ],
    },
    KoKeyword {
        ko: "신발",
        en_tokens: &["shoe", "boot", "sandal", "sneaker"],
    },
    KoKeyword {
        ko: "모자",
        en_tokens: &["hat", "cap", "helmet", "crown"],
    },
    KoKeyword {
        ko: "가방",
        en_tokens: &[
            "bag",
            "purse",
            "handbag",
            "backpack",
            "briefcase",
            "luggage",
        ],
    },
    KoKeyword {
        ko: "안경",
        en_tokens: &["glasses", "goggles", "sunglasses"],
    },
    KoKeyword {
        ko: "보석",
        en_tokens: &["gem", "ring", "diamond"],
    },
    KoKeyword {
        ko: "전자",
        en_tokens: &[
            "computer", "laptop", "keyboard", "mouse", "monitor", "phone", "mobile", "camera",
        ],
    },
    KoKeyword {
        ko: "컴퓨터",
        en_tokens: &["computer", "laptop", "desktop"],
    },
    KoKeyword {
        ko: "휴대폰",
        en_tokens: &["mobile phone"],
    },
    KoKeyword {
        ko: "전화",
        en_tokens: &["telephone", "phone", "pager"],
    },
    KoKeyword {
        ko: "카메라",
        en_tokens: &["camera"],
    },
    KoKeyword {
        ko: "TV",
        en_tokens: &["television"],
    },
    KoKeyword {
        ko: "조명",
        en_tokens: &["bulb", "lamp", "candle", "flashlight"],
    },
    KoKeyword {
        ko: "책",
        en_tokens: &["book"],
    },
    KoKeyword {
        ko: "공책",
        en_tokens: &["notebook", "bookmark", "ledger"],
    },
    KoKeyword {
        ko: "신문",
        en_tokens: &["newspaper"],
    },
    KoKeyword {
        ko: "필기",
        en_tokens: &[
            "pencil",
            "pen",
            "ballpoint",
            "fountain pen",
            "memo",
            "writing",
        ],
    },
    KoKeyword {
        ko: "연필",
        en_tokens: &["pencil"],
    },
    KoKeyword {
        ko: "사무",
        en_tokens: &[
            "briefcase",
            "card index",
            "file folder",
            "calendar",
            "clipboard",
            "paperclip",
            "pin",
            "tape",
        ],
    },
    KoKeyword {
        ko: "메일",
        en_tokens: &["mail", "envelope", "inbox", "outbox", "postbox"],
    },
    KoKeyword {
        ko: "편지",
        en_tokens: &["envelope", "love letter"],
    },
    KoKeyword {
        ko: "돈",
        en_tokens: &[
            "money",
            "dollar",
            "yen",
            "euro",
            "pound",
            "credit card",
            "coin",
        ],
    },
    KoKeyword {
        ko: "자물쇠",
        en_tokens: &["lock", "key", "padlock"],
    },
    KoKeyword {
        ko: "열쇠",
        en_tokens: &["key"],
    },
    KoKeyword {
        ko: "도구",
        en_tokens: &[
            "hammer",
            "wrench",
            "axe",
            "saw",
            "screwdriver",
            "pick",
            "ruler",
            "scale",
        ],
    },
    KoKeyword {
        ko: "총",
        en_tokens: &["pistol", "gun"],
    },
    KoKeyword {
        ko: "무기",
        en_tokens: &["pistol", "knife", "axe", "sword", "dagger", "bow"],
    },
    KoKeyword {
        ko: "약",
        en_tokens: &["pill", "syringe", "stethoscope", "adhesive bandage"],
    },
    KoKeyword {
        ko: "병원",
        en_tokens: &["hospital", "syringe", "stethoscope", "pill"],
    },
    KoKeyword {
        ko: "과학",
        en_tokens: &[
            "test tube",
            "petri",
            "dna",
            "microscope",
            "telescope",
            "satellite",
        ],
    },
    KoKeyword {
        ko: "변기",
        en_tokens: &["toilet"],
    },
    KoKeyword {
        ko: "샤워",
        en_tokens: &["shower", "bathtub", "soap"],
    },
    KoKeyword {
        ko: "휴지",
        en_tokens: &["roll of paper", "tissue"],
    },
    // 기호
    KoKeyword {
        ko: "기호",
        en_tokens: &["symbol", "warning", "arrow", "math", "currency"],
    },
    KoKeyword {
        ko: "확인",
        en_tokens: &["check mark", "ballot"],
    },
    KoKeyword {
        ko: "X",
        en_tokens: &["cross mark"],
    },
    KoKeyword {
        ko: "느낌표",
        en_tokens: &["exclamation"],
    },
    KoKeyword {
        ko: "물음표",
        en_tokens: &["question"],
    },
    KoKeyword {
        ko: "화살표",
        en_tokens: &["arrow"],
    },
    KoKeyword {
        ko: "별자리",
        en_tokens: &[
            "aries",
            "taurus",
            "gemini",
            "cancer",
            "leo",
            "virgo",
            "libra",
            "scorpio",
            "sagittarius",
            "capricorn",
            "aquarius",
            "pisces",
            "ophiuchus",
        ],
    },
    KoKeyword {
        ko: "재활용",
        en_tokens: &["recycling"],
    },
    KoKeyword {
        ko: "주차",
        en_tokens: &["parking"],
    },
    KoKeyword {
        ko: "금지",
        en_tokens: &["no entry", "prohibited", "no smoking"],
    },
    KoKeyword {
        ko: "경고",
        en_tokens: &["warning", "radioactive", "biohazard"],
    },
    KoKeyword {
        ko: "성별",
        en_tokens: &["female sign", "male sign", "transgender"],
    },
    KoKeyword {
        ko: "키캡",
        en_tokens: &["keycap"],
    },
    // 깃발
    KoKeyword {
        ko: "깃발",
        en_tokens: &["flag"],
    },
    KoKeyword {
        ko: "한국",
        en_tokens: &["south korea"],
    },
    KoKeyword {
        ko: "북한",
        en_tokens: &["north korea"],
    },
    KoKeyword {
        ko: "미국",
        en_tokens: &["united states"],
    },
    KoKeyword {
        ko: "일본",
        en_tokens: &["japan"],
    },
    KoKeyword {
        ko: "중국",
        en_tokens: &["china"],
    },
    KoKeyword {
        ko: "영국",
        en_tokens: &["united kingdom", "england", "scotland", "wales"],
    },
    KoKeyword {
        ko: "프랑스",
        en_tokens: &["france"],
    },
    KoKeyword {
        ko: "독일",
        en_tokens: &["germany"],
    },
    KoKeyword {
        ko: "러시아",
        en_tokens: &["russia"],
    },
    KoKeyword {
        ko: "무지개기",
        en_tokens: &["rainbow flag"],
    },
    KoKeyword {
        ko: "해적",
        en_tokens: &["pirate flag"],
    },
];

/// 카테고리 ID를 그대로 받았을 때만 hit. 사용자 편의용.
fn match_category_id(keyword: &str) -> Option<usize> {
    CATEGORIES
        .iter()
        .position(|c| c.id.eq_ignore_ascii_case(keyword))
}

/// 한국어 키워드면 매핑된 영어 토큰 목록을, 아니면 원문 토큰 단일 항목을 돌려줌.
fn expand_keyword(keyword: &str) -> Vec<String> {
    let kw_lower = keyword.to_lowercase();
    let mut tokens: Vec<String> = Vec::new();

    let mut ko_hit = false;
    for entry in KO_KEYWORDS {
        if keyword.contains(entry.ko) || entry.ko.contains(keyword) {
            ko_hit = true;
            for tok in entry.en_tokens {
                tokens.push(tok.to_lowercase());
            }
        }
    }

    if !ko_hit {
        // 한국어 키워드 매핑 실패 → 영어 substring 검색으로 fallback.
        tokens.push(kw_lower);
    }
    tokens
}

/// 키워드로 이모지를 검색합니다.
///
/// 우선순위:
/// 1. 빈 키워드  → `POPULAR_EMOJIS`
/// 2. 카테고리ID → 해당 카테고리 전체 (`Smileys`, `People`, ...)
/// 3. 한국어 키워드 매핑 → 매핑된 영어 토큰을 `en_name`/`subgroup`과 부분 일치
/// 4. 그 외       → 영어 substring 매치 (`en_name` + `subgroup`)
/// 5. 모두 실패   → `POPULAR_EMOJIS` fallback
///
/// 반환값은 `char` 시퀀스. 단, 데이터에는 ZWJ/VS16 등 다중 codepoint emoji가
/// 포함되어 있어 첫 번째 base codepoint만 반환합니다. 풀 시퀀스가 필요한 경우
/// `search_emoji_strings`를 사용하세요.
pub fn search_emoji(keyword: &str) -> Vec<char> {
    search_emoji_strings(keyword)
        .into_iter()
        .filter_map(|s| s.chars().next())
        .collect()
}

/// 키워드 → 이모지 문자열(다중 codepoint 포함) 검색.
pub fn search_emoji_strings(keyword: &str) -> Vec<String> {
    if keyword.is_empty() {
        return POPULAR_EMOJIS.iter().map(|s| s.to_string()).collect();
    }

    // 1) 카테고리 ID 직접 매칭
    if let Some(idx) = match_category_id(keyword) {
        return EMOJIS
            .iter()
            .filter(|e| e.cat_idx as usize == idx)
            .map(|e| e.emoji.to_string())
            .collect();
    }

    // 2) 한국어 매핑 + 영어 substring
    let tokens = expand_keyword(keyword);
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<&'static str> = std::collections::HashSet::new();
    for e in EMOJIS.iter() {
        let name_lc = e.en_name.to_lowercase();
        let sub_lc = e.subgroup.to_lowercase();
        for tok in &tokens {
            if tok.is_empty() {
                continue;
            }
            if name_lc.contains(tok.as_str()) || sub_lc.contains(tok.as_str()) {
                if seen.insert(e.emoji) {
                    out.push(e.emoji.to_string());
                }
                break;
            }
        }
    }

    if out.is_empty() {
        return POPULAR_EMOJIS.iter().map(|s| s.to_string()).collect();
    }
    out
}

/// 카테고리 메타 목록 반환 (DBus용).
///
/// 반환 형식: `Vec<(id, ko_label, en_label, count)>`.
pub fn list_categories() -> Vec<(String, String, String, u32)> {
    CATEGORIES
        .iter()
        .enumerate()
        .map(|(idx, info)| {
            let count = EMOJIS.iter().filter(|e| e.cat_idx as usize == idx).count() as u32;
            (
                info.id.to_string(),
                info.ko.to_string(),
                info.en.to_string(),
                count,
            )
        })
        .collect()
}

/// 카테고리 ID로 이모지 목록 반환.
pub fn category_emojis(category_id: &str) -> Vec<String> {
    match match_category_id(category_id) {
        Some(idx) => EMOJIS
            .iter()
            .filter(|e| e.cat_idx as usize == idx)
            .map(|e| e.emoji.to_string())
            .collect(),
        None => Vec::new(),
    }
}

// ===========================================
// MRU 즐겨찾기 (영속화)
// ===========================================

use std::path::PathBuf;

/// 즐겨찾기 영속화 파일 경로 — `~/.config/unim/emoji-favorites.yaml`
///
/// XDG config dir 식별 실패 시 None.
pub fn favorites_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("unim").join("emoji-favorites.yaml"))
}

/// 즐겨찾기 최대 보관 개수 (MRU 윈도우)
pub const FAVORITES_MAX: usize = 32;

/// 즐겨찾기 읽기 (YAML: 이모지 문자열 배열)
///
/// 파일이 없거나 파싱 실패 시 빈 vec를 반환합니다.
pub fn load_favorites() -> Vec<String> {
    let path = match favorites_path() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    serde_yaml::from_str::<Vec<String>>(&content).unwrap_or_default()
}

/// 즐겨찾기 MRU 갱신 후 저장
///
/// `emoji`를 최상단으로 옮기고(없으면 추가) 최대 `FAVORITES_MAX`개로 잘라낸 뒤
/// 파일에 기록합니다. 파일 쓰기 실패는 무시 (best-effort).
pub fn touch_favorite(emoji: &str) -> Vec<String> {
    let mut list = load_favorites();
    list.retain(|e| e != emoji);
    list.insert(0, emoji.to_string());
    list.truncate(FAVORITES_MAX);

    if let Some(path) = favorites_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(yaml) = serde_yaml::to_string(&list) {
            let _ = std::fs::write(&path, yaml);
        }
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_size_meets_windows_panel_scope() {
        // Windows 11 emoji panel 수준 (~1900±100)
        assert!(
            (1800..=2100).contains(&TOTAL_EMOJIS),
            "TOTAL_EMOJIS={TOTAL_EMOJIS} out of expected window"
        );
        assert_eq!(EMOJIS.len(), TOTAL_EMOJIS);
    }

    #[test]
    fn nine_categories_present() {
        assert_eq!(CATEGORIES.len(), 9);
        let ids: Vec<&str> = CATEGORIES.iter().map(|c| c.id).collect();
        assert_eq!(
            ids,
            vec![
                "Smileys",
                "People",
                "Animals",
                "Food",
                "Travel",
                "Activities",
                "Objects",
                "Symbols",
                "Flags"
            ]
        );
    }

    #[test]
    fn each_category_has_minimum_entries() {
        for (idx, info) in CATEGORIES.iter().enumerate() {
            let n = EMOJIS.iter().filter(|e| e.cat_idx as usize == idx).count();
            // 가장 작은 Activities 카테고리도 60개 이상이어야 함.
            assert!(n >= 60, "category {} has only {} emojis", info.id, n);
        }
    }

    #[test]
    fn search_empty_returns_popular() {
        let r = search_emoji_strings("");
        assert!(!r.is_empty());
        assert!(r.iter().any(|s| s == "😀"));
    }

    #[test]
    fn search_no_match_returns_popular() {
        let r = search_emoji_strings("존재하지않는키워드xyz12345");
        assert!(!r.is_empty());
    }

    #[test]
    fn search_korean_keyword_웃음() {
        let r = search_emoji_strings("웃음");
        assert!(r.len() >= 5, "웃음 search count too low: {}", r.len());
        assert!(r.iter().any(|s| s == "😀"));
    }

    #[test]
    fn search_korean_keyword_사랑_includes_heart() {
        let r = search_emoji_strings("사랑");
        // ❤️ (red heart, fully-qualified U+2764 U+FE0F) — base codepoint U+2764
        assert!(
            r.iter().any(|s| s.starts_with('\u{2764}')),
            "사랑 검색 결과에 빨간 하트 없음"
        );
    }

    #[test]
    fn search_korean_keyword_고양이() {
        let r = search_emoji_strings("고양이");
        assert!(!r.is_empty());
    }

    #[test]
    fn search_english_keyword_pizza() {
        let r = search_emoji_strings("pizza");
        assert!(r.iter().any(|s| s == "🍕"));
    }

    #[test]
    fn search_category_id_returns_full_category() {
        let r = search_emoji_strings("Flags");
        assert!(
            r.len() >= 200,
            "Flags 카테고리 결과가 너무 적음: {}",
            r.len()
        );
    }

    #[test]
    fn list_categories_returns_nine_with_counts() {
        let cats = list_categories();
        assert_eq!(cats.len(), 9);
        for (id, ko, en, count) in &cats {
            assert!(!id.is_empty());
            assert!(!ko.is_empty());
            assert!(!en.is_empty());
            assert!(*count >= 60, "{} count too low: {}", id, count);
        }
    }

    #[test]
    fn category_emojis_smileys_basic() {
        let r = category_emojis("Smileys");
        assert!(r.iter().any(|s| s == "😀"));
        assert!(r.iter().any(|s| s == "😂"));
    }

    #[test]
    fn category_emojis_unknown_returns_empty() {
        let r = category_emojis("UnknownXYZ");
        assert!(r.is_empty());
    }

    #[test]
    fn legacy_search_emoji_returns_chars() {
        // 레거시 char 반환 API가 여전히 동작.
        let r = search_emoji("웃음");
        assert!(!r.is_empty());
    }
}
