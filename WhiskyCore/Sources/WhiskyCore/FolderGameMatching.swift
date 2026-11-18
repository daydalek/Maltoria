//
//  FolderGameMatching.swift
//  WhiskyCore
//
//  This file is part of Whisky.
//
//  Whisky is free software: you can redistribute it and/or modify it under the terms
//  of the GNU General Public License as published by the Free Software Foundation,
//  either version 3 of the License, or (at your option) any later version.
//
//  Whisky is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
//  without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
//  See the GNU General Public License for more details.
//
//  You should have received a copy of the GNU General Public License along with Whisky.
//  If not, see https://www.gnu.org/licenses/.
//

import Foundation

public enum FolderGameMatching {
    private static let shinjitaiMap: [Character: Character] = [
        "桜": "樱", "剣": "剑", "涙": "泪", "姫": "姬", "悪": "恶", "獣": "兽", "黒": "黑", "竜": "龙", "団": "团", "図": "图", "囲": "围",
        "圏": "圈", "発": "发", "髪": "发", "顔": "颜", "頼": "赖", "顕": "显", "駅": "驿", "駆": "驱", "騒": "骚", "験": "验", "髄": "髓",
        "鴎": "鸥", "黙": "默", "齢": "龄", "歩": "步", "気": "气", "沢": "泽", "浜": "滨", "渕": "渊", "満": "满", "滅": "灭", "滝": "泷",
        "漬": "渍", "漸": "渐", "潟": "泻", "渋": "涩", "渓": "溪", "渉": "涉", "猟": "猎", "畳": "叠", "砕": "碎", "禍": "祸", "穂": "穗",
        "箇": "个", "節": "节", "粋": "粹", "粛": "肃", "粧": "妆", "糧": "粮", "経": "经", "絵": "绘", "絶": "绝", "継": "继", "続": "续",
        "総": "总", "緑": "绿", "緒": "绪", "線": "线", "締": "缔", "編": "编", "緩": "缓", "練": "练", "縁": "缘", "縄": "绳", "縦": "纵",
        "縫": "缝", "縮": "缩", "績": "绩", "繊": "纤", "繋": "系", "繍": "绣", "織": "织", "譲": "让", "醸": "酿", "鎌": "镰", "鎮": "镇",
        "鉄": "铁", "鉱": "矿", "鋼": "钢", "録": "录", "銭": "钱", "錬": "炼", "錯": "错", "鍵": "键", "鍛": "锻", "闘": "斗", "陥": "陷",
        "険": "险", "隠": "隐", "隣": "邻", "隷": "隶", "雑": "杂", "難": "难", "霊": "灵", "響": "响", "頂": "顶", "頃": "顷", "順": "顺",
        "須": "须", "預": "预", "頑": "顽", "頓": "顿", "領": "领", "頭": "头", "頬": "颊", "頻": "频", "題": "题", "額": "额", "願": "愿",
        "類": "类", "顧": "顾", "風": "风", "飛": "飞", "飢": "饥", "飯": "饭", "飲": "饮", "飼": "饲", "飽": "饱", "飾": "饰", "餅": "饼",
        "養": "养", "餌": "饵", "餓": "饿", "館": "馆", "駄": "驮", "駐": "驻", "駒": "驹", "騎": "骑", "騰": "腾", "驚": "惊", "鶏": "鸡",
        "鶴": "鹤", "鷲": "鹫", "鷹": "鹰", "麗": "丽", "麺": "面", "斉": "齐", "歯": "齿", "亀": "龟", "転": "转", "伝": "传", "芸": "艺",
        "広": "广", "応": "应", "帰": "归", "変": "变", "楽": "乐", "様": "样", "権": "权", "歓": "欢", "殻": "壳", "毎": "每", "氷": "冰",
        "決": "决", "況": "况", "済": "济", "涼": "凉", "渇": "渴", "為": "为", "焼": "烧", "熱": "热", "愛": "爱", "獄": "狱", "現": "现",
        "産": "产", "異": "异", "疎": "疏", "監": "监", "盤": "盘", "県": "县", "矯": "矫", "砲": "炮", "確": "确", "礎": "础", "稲": "稻",
        "穀": "谷", "積": "积", "穏": "稳", "穫": "获", "窓": "窗", "窮": "穷", "窯": "窑", "競": "竞", "処": "处", "対": "对", "実": "实",
        "辺": "边", "郷": "乡", "嬢": "娘", "専": "专", "単": "单", "厳": "严", "呪": "咒", "営": "营", "嘆": "叹", "圧": "压", "塁": "垒",
        "塩": "盐", "増": "增", "壊": "坏", "売": "卖", "奨": "奖", "娯": "娱", "寛": "宽", "層": "层", "帯": "带", "廃": "废", "乗": "乘",
        "亜": "亚", "仏": "佛", "仮": "假", "児": "儿", "円": "圆", "剤": "剂", "剰": "剩", "労": "劳", "効": "效", "勧": "劝", "勲": "勋",
        "釈": "释", "酔": "醉", "鋭": "锐", "鋳": "铸", "鎖": "锁", "関": "关", "閲": "阅", "闇": "暗", "姉": "姐", "暁": "晓", "栄": "荣",
        "検": "检", "歳": "岁", "浄": "净", "暦": "历", "聴": "听", "脳": "脑", "臓": "脏", "舎": "舍", "舗": "铺", "薫": "薰", "薬": "药",
        "蛍": "萤", "覚": "觉", "観": "观", "訳": "译", "証": "证", "読": "读", "豊": "丰", "軽": "轻", "輪": "轮", "輸": "输", "遅": "迟",
        "選": "选", "遺": "遗", "還": "还", "郵": "邮", "針": "针", "銀": "银", "銅": "铜", "鏡": "镜", "鐘": "钟", "鑑": "鉴", "陰": "阴",
        "陽": "阳", "陸": "陆", "隊": "队", "階": "阶", "離": "离", "雲": "云", "電": "电", "霧": "雾", "頒": "颁", "両": "两", "並": "并",
        "巣": "巢", "庁": "厅", "弁": "辩", "弐": "贰", "弾": "弹", "従": "从", "徴": "征", "徳": "德", "恵": "惠", "悩": "恼", "態": "态",
        "慣": "惯", "懐": "怀", "戦": "战", "戯": "戏", "抜": "拔", "択": "择", "拝": "拜", "拠": "据", "拡": "扩", "挙": "举", "挿": "插",
        "捜": "搜", "捨": "舍", "掃": "扫", "掲": "揭", "換": "换", "揮": "挥", "揺": "摇", "損": "损", "摂": "摄", "撃": "击", "擬": "拟",
        "敵": "敌", "斎": "斋", "時": "时", "晩": "晚", "曇": "昙", "書": "书", "東": "东", "査": "查", "桟": "栈", "棄": "弃", "棟": "栋",
        "業": "业", "極": "极", "構": "构", "標": "标", "樹": "树", "橋": "桥", "機": "机", "欄": "栏", "殺": "杀", "漢": "汉", "潔": "洁",
        "潤": "润", "濁": "浊", "濃": "浓", "濫": "滥", "瀬": "濑", "災": "灾", "煙": "烟", "無": "无", "犠": "牺", "猶": "犹", "獲": "获",
        "療": "疗", "癒": "愈", "種": "种", "筆": "笔", "範": "范", "築": "筑", "簡": "简", "紅": "红", "納": "纳", "純": "纯", "紙": "纸",
        "級": "级", "紛": "纷", "紡": "纺", "細": "细", "終": "终", "組": "组", "結": "结", "絞": "绞", "絡": "络", "給": "给", "統": "统",
        "絹": "绢", "維": "维", "綱": "纲", "網": "网", "綿": "绵", "緊": "紧", "縛": "缚", "繕": "缮", "繭": "茧", "罰": "罚", "義": "义",
        "習": "习", "聞": "闻", "脈": "脉", "腸": "肠", "膚": "肤", "臨": "临", "興": "兴", "艦": "舰", "艶": "艳", "荘": "庄", "華": "华",
        "葉": "叶", "蔵": "藏", "虜": "虏", "衆": "众", "術": "术", "衛": "卫", "衝": "冲", "裏": "里", "補": "补", "複": "复", "襲": "袭",
        "覇": "霸", "見": "见", "規": "规", "視": "视", "覧": "览", "親": "亲", "訂": "订", "計": "计", "討": "讨", "訓": "训", "託": "托",
        "記": "记", "設": "设", "許": "许", "訴": "诉", "診": "诊", "詐": "诈", "評": "评", "詞": "词", "詠": "咏", "試": "试", "詩": "诗",
        "詰": "诘", "話": "话", "該": "该", "詳": "详", "誇": "夸", "誌": "志", "認": "认", "誕": "诞", "誘": "诱", "語": "语", "誠": "诚",
        "誤": "误", "説": "说", "誰": "谁", "課": "课", "調": "调", "談": "谈", "請": "请", "論": "论", "諦": "谛", "諸": "诸", "諾": "诺",
        "謀": "谋", "謁": "谒", "謎": "谜", "謙": "谦", "講": "讲", "謝": "谢", "識": "识", "議": "议", "護": "护", "貝": "贝", "貞": "贞",
        "負": "负", "財": "财", "貢": "贡", "貧": "贫", "貨": "货", "販": "贩", "貪": "贪", "貫": "贯", "責": "责", "貯": "贮", "貴": "贵",
        "買": "买", "貸": "贷", "費": "费", "貿": "贸", "賀": "贺", "賃": "赁", "資": "资", "賊": "贼", "賛": "赞", "賜": "赐", "賞": "赏",
        "賠": "赔", "賢": "贤", "賦": "赋", "質": "质", "賭": "赌", "購": "购", "贈": "赠", "躍": "跃", "車": "车", "軌": "轨", "軍": "军",
        "軒": "轩", "軟": "软", "較": "较", "載": "载", "輝": "辉", "輩": "辈", "農": "农", "週": "周", "進": "进", "釣": "钓", "鈍": "钝",
        "鈴": "铃", "銃": "铳", "銘": "铭", "鍋": "锅", "長": "长", "門": "门", "閉": "闭", "開": "开", "間": "间", "閣": "阁", "陳": "陈",
        "際": "际", "韓": "韩", "韻": "韵", "項": "项", "戸": "户", "戻": "戾", "払": "拂"
    ]

    private static let translationGroupMarkers =
        #"汉化组|漢化組|汉化补丁|漢化補丁|汉化|漢化|翻译|翻訳|字幕组|字幕組|移植|精翻|机翻|機翻|润色|潤色"#
            + #"|硬盘版|硬盤版|个人|個人|联合|聯合|修正|补丁|補丁|压制|壓制|整理|合集"#

    private static let studioStructuralMarkers =
        #"ソフト|ワークス|スタジオ|プロジェクト|システム|製作所|制作所|企画|開発|制作|株式会社|工房|商会"#
            + #"|(?<![a-z])(?:works|studio|project|team|circle|software|entertainment)(?![a-z])|soft(?![a-z])"#

    private static let knownStudios =
        #"ゆずソフト|key|type-moon|typemoon|leaf|aquaplus|august|nitroplus|nitro\+|minori|alicesoft|アリスソフト"#
            + #"|eushully|エウシュリー|lump\s*of\s*sugar|hooksoft|smee|asa\s*project|asaproject|palette|favorite|whirlpool"#
            + #"|purple\s*software|feng|nana\s*wind|nanawind|cube|hibiki|madosoft|まどそふと|hulotte|yuzusoft"#
            + #"|saga\s*planets|frontwing|front\s*wing|minatosoft|light|07th\s*expansion|sprite|nekoworks|makura|枕"#
            + #"|keroq|ケロq|propeller|lilith|clockup|pulltop|windmill|ういんどみる|ensemble|moonstone|axl"#
            + #"|tone\s*work['’]?s|circus|overflow|innocent\s*grey|laplacian|ラプラシアン|lose"#

    private static let knownStudiosAndGroups = knownStudios
        + #"|心愿屋|心願屋|默示|澄空|华盟|華盟|脸肿|臉腫|雪月花|真白|终点|終點|绯月|緋月|御宅|苍雪|蒼雪|萌幻|失落小站"#
        + #"|天使二次元|花火|桐叶|桐葉|忧郁的弟弟|憂鬱的弟弟|kfc|白杨|白楊|初音|梦想|夢想|galzy"#

    private static let dateTagPattern = #"(?:19|20)\d{2}[.\-_]\d{1,2}[.\-_]\d{1,2}|(?:19|20)?\d{6}"#
    private static let storeEventIDPattern = #"[rvb]j\d+|c\d{2,3}(?:\s*版)?"#

    private static let leadingTagTokenPattern =
        knownStudiosAndGroups
            + #"|"# + dateTagPattern
            + #"|"# + storeEventIDPattern
            + #"|18禁(?:ゲーム)?|r-?18\+?|エロゲ|同人(?:ゲーム)?|美少女ゲーム|ビジュアルノベル|adv|rpg|slg|gal|vn"#
            + #"|krkr2?|kirikiri2?|ons|renpy|bgi|ethornell|siglus|reallive|artemis|qlie|yu-ris|yuris|majiro"#
            + #"|catssystem2?|cs2|wolf|rpgmv|rpgmz|unity|pc(?:版)?|dl版|パッケージ版|steam版|dlsite版|fanza版|dmm版"#
            + #"|gog版|win\d*|windows"#

    private static let bareSuffixPattern =
        #"官中(?:版|补丁|補丁)?"#
            + #"|(?:(?:ai|gpt)(?![a-z])|云|雲|精|机|機|人工|个人|個人|内嵌|內嵌|外挂|外掛|整合|修正|润色|潤色)?"#
            + #"(?:精翻|汉化|漢化|翻译|翻訳|机翻|機翻)(?:绿色|綠色|免安装|免安裝|硬盘|硬盤|润色|潤色)?(?:版|补丁|補丁)?"#
            + #"|(?:官方)?(?:简体中文|繁体中文|简繁中文|簡繁中文|简中|簡中|繁中|中文)"#
            + #"(?:绿色|綠色|免安装|免安裝|硬盘|硬盤)?(?:版|补丁|補丁)?"#
            + #"|(?:免安装|免安裝|硬盘|硬盤|绿色|綠色|无修|無修|步兵|全cg|全特典|整合|内嵌|內嵌|体验|体験|试玩|試玩"#
            + #"|便携|便攜|直装|直裝|解压即玩|解壓即玩|免dvd|全回想|全解锁|全解鎖|含存档|含存檔|带存档|帶存檔"#
            + #"|含攻略|带攻略|帶攻略|去码|去碼|解码|解碼|反和谐|反和諧|全语音|全語音"#
            + #"|(?<![a-z])full\s*voice(?![a-z])|语音补丁|語音補丁)"#
            + #"(?:绿色|綠色|免安装|免安裝|硬盘|硬盤)?(?:版|补丁|補丁)?"#
            + #"|(?<![a-z])(?:通常|初回(?:限定)?|限定|豪华|豪華|特典|dl|steam|dlsite|fanza|dmm|gog"#
            + #"|パッケージ|移植|pc|win\d*|windows)版"#
            + #"|(?<![a-z0-9])r-?18(?:\+|补丁|補丁)?"#
            + #"|(?<![a-z0-9])(?:v\d+(?:\.\d+)+(?:[a-z])?|(?:ver\.?|version|build)\s*\d+(?:\.\d+)*(?:[a-z])?)"#

    private static let bracketedTrailingTokenPattern =
        bareSuffixPattern
            + #"|jpn|ja|eng|ko|kr|chs|cht|sc|tc|cn|jp|en|pc|dl版|パッケージ版|krkr2?|ons|kirikiri2?"#
            + #"|renpy|bgi|siglus|artemis|qlie|majiro|catssystem2?|wolf|rpgmv|rpgmz|unity"#
            + #"|18禁ゲーム|エロゲ|同人ゲーム|18\+|全年龄|全年齡|all[- ]ages?|full\s*voice"#
            + #"|no\s*mosaic|uncensored|decensored|fhd|hd|1080p|720p|4k|60fps|rip|nodvd|crack|iso"#
            + #"|"# + storeEventIDPattern
            + #"|"# + dateTagPattern
            + #"|v\d+(?:\.\d+)*(?:[a-z])?"#

    private static let leadingPrefixRegex = try? NSRegularExpression(
        pattern: #"^(?:\[([^\]]+)\]|【([^】]+)】|\(([^)]+)\)|（([^）]+)）)\s*(?=\S)"#
    )
    private static let leadingMarkerRegex = try? NSRegularExpression(
        pattern: "(?i)(?:" + translationGroupMarkers + "|" + studioStructuralMarkers + ")"
    )
    private static let knownStudioRegex = try? NSRegularExpression(pattern: "(?i)^(?:" + knownStudios + ")$")
    private static let knownStudioTokenRegex = try? NSRegularExpression(
        pattern: #"(?i)(?<![^\s/&+×・,])(?:"# + knownStudios + #")(?=$|[\s/&+×・,])"#
    )
    private static let studioMarkerRegex = try? NSRegularExpression(pattern: "(?i)(?:" + studioStructuralMarkers + ")")
    private static let translationMarkerRegex = try? NSRegularExpression(
        pattern: "(?i)(?:" + translationGroupMarkers + ")"
    )
    private static let leadingTagTokensRegex = try? NSRegularExpression(
        pattern: "(?i)^(?:" + leadingTagTokenPattern + ")(?:\\s*[/&+×・,\\s]\\s*(?:" + leadingTagTokenPattern + "))*$"
    )
    private static let trailingBracketRegex = try? NSRegularExpression(
        pattern: #"^(.+?)\s*(?:\[([^\]]+)\]|【([^】]+)】|\(([^)]+)\)|（([^）]+)）)\s*$"#
    )
    private static let trailingBracketTokensRegex = try? NSRegularExpression(
        pattern: "(?i)^(?:" + bracketedTrailingTokenPattern + ")"
            + "(?:\\s*[/&+×・,\\s]\\s*(?:" + bracketedTrailingTokenPattern + "))*$"
    )
    private static let trailingTranslationTagRegex = try? NSRegularExpression(
        pattern: #"(?i)(?:汉化组|漢化組|汉化补丁|漢化補丁|汉化|漢化|翻译|翻訳|字幕组|字幕組|机翻|機翻)"#
    )
    private static let trailingBareSuffixRegex = try? NSRegularExpression(
        pattern: "(?i)(?:^|\\s*)(?:" + bareSuffixPattern + ")\\s*$"
    )

    private static let editionTokenReplacements: [String: String] = [
        "ii": "2", "iii": "3", "iv": "4", "vi": "6", "vii": "7", "viii": "8", "ix": "9",
        "ファンディスク": "fd", "fandisc": "fd", "fan disc": "fd",
        "リメイク": "remake", "重制": "remake", "重製": "remake", "重置": "remake",
        "重制版": "remake", "重製版": "remake", "重置版": "remake",
        "リマスター": "remaster", "remastered": "remaster",
        "完全版": "complete", "完整版": "complete"
    ]

    private static let chapterSubtitleRegex = try? NSRegularExpression(
        pattern: #"(?i)(?:前[編篇]|後編|[后後][編篇]|[上下][編篇巻卷]|序章|[終终]章|第.[章話话幕部]"#
            + #"|(?<![a-z])(?:chapter|episode|ep|part|vol|act|introductory|closing|after|prologue|epilogue)(?![a-z]))"#
    )
    private static let explicitChapterRegex = try? NSRegularExpression(
        pattern: #"(?i)(?:前[編篇]|後編|[后後][編篇]|[上下][編篇巻卷]|序章|[終终]章"#
            + #"|第\s*[0-9０-９一二三四五六七八九十]+\s*[章話话幕部巻卷]"#
            + #"|(?<![a-z])(?:chapter|episode|ep|part|vol|act)\.?\s*[0-9０-９]+)"#
    )

    /// Remove known packaging prefixes/suffixes, not arbitrary bracketed subtitles.
    public static func query(_ name: String) -> String {
        let strippedLeading = stripLeadingNoisePrefixes(name.trimmingCharacters(in: .whitespacesAndNewlines))
        return stripTrailingNoiseSuffixes(strippedLeading)
    }

    /// Extract up to 3 prioritized, deduplicated queries from a folder or release name.
    public static func searchQueries(_ name: String) -> [String] {
        let cleaned = query(name)
        guard !cleaned.isEmpty else { return [] }
        var queries: [String] = []

        func appendUnique(_ candidate: String) {
            let trimmed = candidate.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty, queries.count < 3, !queries.contains(trimmed) else { return }
            queries.append(trimmed)
        }

        appendUnique(cleaned)
        let base = stripLeadingBracketPrefixes(cleaned)?.body ?? cleaned
        if let (left, right) = splitBilingualDelimited(base) {
            appendUnique(left)
            appendUnique(right)
        } else if let (outer, inner) = splitBilingualParenthetical(base) {
            appendUnique(outer)
            appendUnique(inner)
        } else {
            if base != cleaned {
                appendUnique(base)
            }
            if let (main, _) = stripSubtitle(base),
               edition(main) == edition(base),
               meetsScriptLength(normalized(main), minCJK: 2, minLatin: 5) {
                appendUnique(main)
            }
        }
        return queries
    }

    /// Extract clean local display title for VNDB non-Chinese entries.
    static func localDisplayTitle(_ name: String) -> String {
        let cleaned = query(name)
        let unprefixed = stripLeadingBracketPrefixes(cleaned)?.body ?? cleaned
        if let (outer, _) = splitBilingualParenthetical(unprefixed) {
            return outer
        }
        return unprefixed
    }

    public static func normalized(_ text: String) -> String {
        let simplified = text.applyingTransform(StringTransform("Traditional-Simplified"), reverse: false) ?? text
        let unified = String(simplified.map { shinjitaiMap[$0] ?? $0 })
        let expanded = expandIterationMarks(unified)
        let arabicRomans = replaceUnicodeRomanNumerals(expanded)
        var folded = arabicRomans.folding(
            options: [.caseInsensitive, .diacriticInsensitive, .widthInsensitive],
            locale: Locale(identifier: "en_US_POSIX")
        )
        let replacements: [(String, String)] = [
            (#"(?<![a-z])ii(?![a-z])"#, "2"), (#"(?<![a-z])iii(?![a-z])"#, "3"),
            (#"(?<![a-z])iv(?![a-z])"#, "4"), (#"(?<![a-z])vi(?![a-z])"#, "6"),
            (#"(?<![a-z])vii(?![a-z])"#, "7"), (#"(?<![a-z])viii(?![a-z])"#, "8"),
            (#"(?<![a-z])ix(?![a-z])"#, "9"),
            (#"(?<![a-z])remastered(?![a-z])|リマスター"#, "remaster"),
            (#"fandisc|fan\s*disc|ファンディスク"#, "fd"),
            (#"リメイク|(?:重制|重製|重置)(?:版)?"#, "remake"),
            (#"(?:完全|完整)版"#, "complete")
        ]
        for (pattern, replacement) in replacements {
            folded = folded.replacingOccurrences(of: pattern, with: replacement, options: .regularExpression)
        }
        return folded.unicodeScalars.filter { CharacterSet.alphanumerics.contains($0) }.map(String.init).joined()
    }

    static func galgameCanonical(_ text: String) -> String {
        let base = normalized(text)
        var scalars: [UnicodeScalar] = []
        scalars.reserveCapacity(base.unicodeScalars.count)
        for scalar in base.unicodeScalars {
            if scalar.value == 0x30FC {
                continue
            }
            let mapped: UnicodeScalar = if (0x30A1 ... 0x30F6).contains(scalar.value),
                                           let hiragana = UnicodeScalar(scalar.value - 0x60) {
                hiragana
            } else {
                scalar
            }
            if mapped.value == 0x306E || mapped.value == 0x7684,
               let unifiedParticle = UnicodeScalar(0x4E4B) {
                scalars.append(unifiedParticle)
            } else {
                scalars.append(mapped)
            }
        }
        return String(String.UnicodeScalarView(scalars))
    }

    /// All names participate, while display language remains independent.
    public static func ranked(query: String, games: [GameMetadata]) -> [GameMetadata] {
        games.enumerated().sorted {
            let lhs = rankingScore(query: query, game: $0.element)
            let rhs = rankingScore(query: query, game: $1.element)
            return lhs == rhs ? $0.offset < $1.offset : lhs > rhs
        }.map(\.element)
    }

    public static func exactMatches(query: String, in games: [GameMetadata]) -> [GameMetadata] {
        let cleaned = self.query(query)
        let needle = normalized(cleaned)
        guard !needle.isEmpty else { return [] }
        return games.filter {
            hasCompatibleDeveloper(query: query, game: $0)
                && hasCompatibleChapter(query: cleaned, game: $0)
                && isExactMatch(query: cleaned, game: $0)
        }
    }

    /// Prefer exact identities across sources, then accept only a clearly leading
    /// long-name near-match. Source errors leave other providers available.
    public static func candidate(
        query: String,
        results: [GameMetadataSearchResult],
        sources: [GameMetadataSource]
    ) -> GameMetadata? {
        let needle = normalized(query)
        guard !needle.isEmpty else { return nil }
        let available = sources.compactMap { source in
            results.first { $0.source == source && $0.error == nil }
        }
        for result in available {
            let matches = exactMatches(query: query, in: result.games)
            if matches.count > 1 {
                return nil
            }
            if let match = matches.first {
                return match
            }
        }
        return highConfidenceFuzzyMatch(query: query, in: available.flatMap(\.games))
    }

    private struct ScoredCandidate {
        let index: Int
        let game: GameMetadata
        let score: Double
    }

    private static func highConfidenceFuzzyMatch(query: String, in games: [GameMetadata]) -> GameMetadata? {
        let cleaned = self.query(query)
        let candidates = games.enumerated().compactMap { index, game -> ScoredCandidate? in
            guard hasCompatibleDeveloper(query: query, game: game),
                  hasCompatibleChapter(query: cleaned, game: game)
            else { return nil }
            return ScoredCandidate(index: index, game: game, score: score(query: cleaned, game: game))
        }.sorted { lhs, rhs in
            lhs.score == rhs.score ? lhs.index < rhs.index : lhs.score > rhs.score
        }
        guard let first = candidates.first, first.score >= 0.92 else { return nil }
        var sameWorkCandidates = [first.game]
        for candidate in candidates.dropFirst() where first.score - candidate.score < 0.12 {
            guard sameWorkCandidates.allSatisfy({ sameWork($0, candidate.game) }) else { return nil }
            sameWorkCandidates.append(candidate.game)
        }
        return first.game
    }

    private static func sameWork(_ lhs: GameMetadata, _ rhs: GameMetadata) -> Bool {
        if lhs.id == rhs.id {
            return true
        }
        guard lhs.source != rhs.source,
              !lhs.developer.isEmpty, !rhs.developer.isEmpty,
              studioMatches(lhs.developer, rhs.developer)
        else { return false }
        let lhsYear = lhs.releaseDate.prefix(4)
        let rhsYear = rhs.releaseDate.prefix(4)
        if lhsYear.count == 4, rhsYear.count == 4,
           lhsYear.allSatisfy(\.isNumber), rhsYear.allSatisfy(\.isNumber), lhsYear != rhsYear {
            return false
        }
        let lhsTitles = [lhs.title, lhs.originalTitle].filter { !$0.isEmpty }
        let rhsTitles = [rhs.title, rhs.originalTitle].filter { !$0.isEmpty }
        return lhsTitles.contains { left in
            rhsTitles.contains { right in
                titlesMatchExactly(left, right) && chaptersCompatible(left, right)
            }
        }
    }

    // MARK: - Exact & Fuzzy Scoring

    private static func isExactMatch(query: String, game: GameMetadata) -> Bool {
        let variants = titleVariants(for: game)
        if variants.contains(where: { titlesMatchExactly(query, $0) }) {
            return true
        }
        if let (prefixes, body) = stripLeadingBracketPrefixes(query) {
            if prefixes.contains(where: { developerMatches($0, game.developer) }),
               variants.contains(where: { titlesMatchExactly(body, $0) }) {
                return true
            }
            if prefixes.contains(where: { developerMatches($0, game.developer) }),
               let (partA, partB) = splitBilingual(body),
               variants.contains(where: { titlesMatchExactly(partA, $0) }),
               variants.contains(where: { titlesMatchExactly(partB, $0) }) {
                return true
            }
        }
        if let (partA, partB) = splitBilingual(query),
           variants.contains(where: { titlesMatchExactly(partA, $0) }),
           variants.contains(where: { titlesMatchExactly(partB, $0) }) {
            return true
        }
        return false
    }

    private static func titlesMatchExactly(_ lhs: String, _ rhs: String) -> Bool {
        guard edition(lhs) == edition(rhs) else { return false }
        let normL = normalized(lhs)
        let normR = normalized(rhs)
        guard !normL.isEmpty, !normR.isEmpty else { return false }
        return normL == normR || galgameCanonical(lhs) == galgameCanonical(rhs)
    }

    private static func score(query: String, game: GameMetadata) -> Double {
        if isExactMatch(query: query, game: game) {
            return 1.0
        }
        let variants = titleVariants(for: game)
        var best = 0.0

        // Bilingual single-side exact match (0.96)
        if stripLeadingBracketPrefixes(query) == nil, let (partA, partB) = splitBilingual(query) {
            for part in [partA, partB] where meetsScriptLength(normalized(part), minCJK: 2, minLatin: 5) {
                if variants.contains(where: { chaptersCompatible(part, $0) && titlesMatchExactly(part, $0) }) {
                    best = max(best, 0.96)
                }
            }
        }

        // Main-title (subtitle-stripped) exact match (0.95)
        if variants.contains(where: { chaptersCompatible(query, $0) && isMainTitleMatch(query, $0) }) {
            best = max(best, 0.95)
        }

        // CJK weighted edit similarity
        for variant in variants where chaptersCompatible(query, variant) {
            best = max(best, weightedCandidateSimilarity(query, variant))
        }
        return best
    }

    private static func isMainTitleMatch(_ query: String, _ titleVariant: String) -> Bool {
        guard edition(query) == edition(titleVariant) else { return false }
        let querySplit = stripSubtitle(query)
        let titleSplit = stripSubtitle(titleVariant)
        guard querySplit != nil || titleSplit != nil else { return false }
        let queryMain = querySplit?.main ?? query
        let titleMain = titleSplit?.main ?? titleVariant
        guard edition(queryMain) == edition(titleMain) else { return false }

        let normQ = normalized(queryMain)
        let normT = normalized(titleMain)
        guard !normQ.isEmpty,
              meetsScriptLength(normQ, minCJK: 3, minLatin: 6),
              meetsScriptLength(normT, minCJK: 3, minLatin: 6),
              normQ == normT || galgameCanonical(queryMain) == galgameCanonical(titleMain)
        else { return false }

        if let subQ = querySplit?.subtitle, let subT = titleSplit?.subtitle {
            let normSubQ = normalized(subQ)
            let normSubT = normalized(subT)
            if normSubQ != normSubT {
                if !haveDifferentScripts(subQ, subT)
                    || containsChapterSubtitleMarker(subQ)
                    || containsChapterSubtitleMarker(subT) {
                    return false
                }
            }
        }
        return true
    }

    private static func weightedCandidateSimilarity(_ query: String, _ title: String) -> Double {
        guard edition(query) == edition(title) else { return 0.0 }
        let normBest = pairWeightedSimilarity(normalized(query), normalized(title))
        let canonicalBest = pairWeightedSimilarity(galgameCanonical(query), galgameCanonical(title))
        return max(normBest, canonicalBest)
    }

    private static func pairWeightedSimilarity(_ lhs: String, _ rhs: String) -> Double {
        let minRequired = containsCJK(lhs) && containsCJK(rhs) ? 5 : 8
        guard min(lhs.count, rhs.count) >= minRequired,
              max(lhs.count, rhs.count) <= 256
        else { return 0.0 }
        return weightedEditSimilarity(lhs, rhs)
    }

    private static func rankingScore(query: String, game: GameMetadata) -> Double {
        let variants = titleVariants(for: game)
        var candidateQueries = [query]
        if let (_, body) = stripLeadingBracketPrefixes(query) {
            candidateQueries.append(body)
        }
        if let (partA, partB) = splitBilingual(stripLeadingBracketPrefixes(query)?.body ?? query) {
            candidateQueries.append(partA)
            candidateQueries.append(partB)
        }
        var best = 0.0
        for (index, currentQuery) in candidateQueries.enumerated() {
            let queryFactor = index == 0 ? 1.0 : 0.96
            for variant in variants {
                best = max(best, rankingPairScore(currentQuery, variant) * queryFactor)
            }
        }
        return best
    }

    private static func rankingPairScore(_ query: String, _ title: String) -> Double {
        let needle = normalized(query)
        let name = normalized(title)
        guard !needle.isEmpty, !name.isEmpty, max(name.count, needle.count) <= 256 else { return 0.0 }
        let editionPenalty = edition(query) == edition(title) ? 1.0 : 0.75
        let needleCan = galgameCanonical(query)
        let nameCan = galgameCanonical(title)
        if name == needle || nameCan == needleCan {
            return 1.0 * editionPenalty
        }

        var raw = max(weightedEditSimilarity(needle, name), weightedEditSimilarity(needleCan, nameCan))
        if isMainTitleMatch(query, title) {
            raw = max(raw, 0.95)
        } else {
            let mainQ = stripSubtitle(query)?.main ?? query
            let mainT = stripSubtitle(title)?.main ?? title
            if mainQ != query || mainT != title {
                let mainSim = max(
                    weightedEditSimilarity(normalized(mainQ), normalized(mainT)),
                    weightedEditSimilarity(galgameCanonical(mainQ), galgameCanonical(mainT))
                )
                raw = max(raw, mainSim * 0.93)
            }
        }

        let coverage = Double(min(needle.count, name.count)) / Double(max(needle.count, name.count))
        if name.hasPrefix(needle) || needle.hasPrefix(name)
            || nameCan.hasPrefix(needleCan) || needleCan.hasPrefix(nameCan) {
            raw = max(raw, 0.70 + 0.22 * coverage)
        } else if min(needle.count, name.count) >= 2,
                  name.contains(needle) || needle.contains(name)
                  || nameCan.contains(needleCan) || needleCan.contains(nameCan) {
            raw = max(raw, 0.55 + 0.25 * coverage)
        }

        let tokenOverlap = tokenDiceSimilarity(query, title)
        if tokenOverlap > 0 {
            raw = max(raw, 0.85 * tokenOverlap)
        }
        return raw * editionPenalty
    }

    private static func tokenDiceSimilarity(_ lhs: String, _ rhs: String) -> Double {
        let leftTokens = normalizedTokens(lhs)
        let rightTokens = normalizedTokens(rhs)
        guard !leftTokens.isEmpty, !rightTokens.isEmpty else { return 0 }
        let intersection = leftTokens.intersection(rightTokens).count
        guard intersection > 0 else { return 0 }
        return Double(2 * intersection) / Double(leftTokens.count + rightTokens.count)
    }

    private static func normalizedTokens(_ text: String) -> Set<String> {
        let simplified = text.applyingTransform(StringTransform("Traditional-Simplified"), reverse: false) ?? text
        let unified = String(simplified.map { shinjitaiMap[$0] ?? $0 })
        let expanded = expandIterationMarks(unified)
        let arabic = replaceUnicodeRomanNumerals(expanded)
        let folded = arabic.folding(
            options: [.caseInsensitive, .diacriticInsensitive, .widthInsensitive],
            locale: Locale(identifier: "en_US_POSIX")
        )
        let parts = folded.components(separatedBy: CharacterSet.alphanumerics.inverted)
            .filter { !$0.isEmpty }
            .map { normalized($0) }
            .filter { !$0.isEmpty }
        return Set(parts)
    }

    private static func weightedEditSimilarity(_ lhsText: String, _ rhsText: String) -> Double {
        let lhs = Array(lhsText), rhs = Array(rhsText)
        guard !lhs.isEmpty, !rhs.isEmpty else { return 0 }
        var previous = Array(0 ... rhs.count)
        for (rowIndex, left) in lhs.enumerated() {
            var current = [rowIndex + 1] + Array(repeating: 0, count: rhs.count)
            for (colIndex, right) in rhs.enumerated() {
                current[colIndex + 1] = min(
                    current[colIndex] + 1,
                    previous[colIndex + 1] + 1,
                    previous[colIndex] + (left == right ? 0 : 1)
                )
            }
            previous = current
        }
        let maxWeight = max(effectiveLength(lhsText), effectiveLength(rhsText))
        guard maxWeight > 0 else { return 0 }
        return max(0.0, 1.0 - Double(previous[rhs.count]) / maxWeight)
    }

    private static func effectiveLength(_ text: String) -> Double {
        text.unicodeScalars.reduce(0.0) { total, scalar in
            total + ((0x3040 ... 0x9FFF).contains(scalar.value) ? 2.6 : 1.0)
        }
    }

    // MARK: - Title Decomposition & Helpers

    private static func titleVariants(for game: GameMetadata) -> [String] {
        var seen = Set<String>()
        var variants: [String] = []
        func append(_ item: String) {
            let trimmed = item.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty, seen.insert(trimmed).inserted else { return }
            variants.append(trimmed)
        }
        for title in game.matchingTitles {
            append(title)
            for separator in [" // ", " / ", " ／ ", " | ", " ｜ "] where title.contains(separator) {
                let parts = title.components(separatedBy: separator)
                    .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                if parts.count >= 2, parts.allSatisfy({ !$0.isEmpty }) {
                    parts.forEach(append)
                }
            }
        }
        return variants
    }

    private static func splitBilingual(_ text: String) -> (String, String)? {
        splitBilingualDelimited(text) ?? splitBilingualParenthetical(text)
    }

    private static func splitBilingualDelimited(_ text: String) -> (String, String)? {
        for separator in [" // ", " / ", " ／ ", " | ", " ｜ "] {
            guard let range = text.range(of: separator) else { continue }
            let left = String(text[..<range.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
            let right = String(text[range.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
            if !left.isEmpty, !right.isEmpty {
                return (left, right)
            }
        }
        return nil
    }

    private static func splitBilingualParenthetical(_ text: String) -> (String, String)? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let last = trimmed.last, last == ")" || last == "）" else { return nil }
        let open: Character = last == ")" ? "(" : "（"
        guard let openIndex = trimmed.lastIndex(of: open), openIndex > trimmed.startIndex else { return nil }
        let outer = String(trimmed[..<openIndex]).trimmingCharacters(in: .whitespacesAndNewlines)
        let innerStart = trimmed.index(after: openIndex)
        let innerEnd = trimmed.index(before: trimmed.endIndex)
        guard innerStart < innerEnd else { return nil }
        let inner = String(trimmed[innerStart ..< innerEnd]).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !outer.isEmpty, !inner.isEmpty,
              edition(inner).isEmpty,
              haveDifferentScripts(outer, inner)
        else { return nil }
        return (outer, inner)
    }

    private static func stripLeadingBracketPrefixes(_ text: String) -> (prefixes: [String], body: String)? {
        var remaining = text.trimmingCharacters(in: .whitespacesAndNewlines)
        var prefixes: [String] = []
        while let first = remaining.first, first == "[" || first == "【" {
            let close: Character = first == "[" ? "]" : "】"
            guard let closeIndex = remaining.firstIndex(of: close) else { break }
            let innerStart = remaining.index(after: remaining.startIndex)
            let prefix = String(remaining[innerStart ..< closeIndex]).trimmingCharacters(in: .whitespacesAndNewlines)
            let rest = String(remaining[remaining.index(after: closeIndex)...])
                .trimmingCharacters(in: .whitespacesAndNewlines)
            guard !prefix.isEmpty, !rest.isEmpty else { break }
            prefixes.append(prefix)
            remaining = rest
        }
        return prefixes.isEmpty ? nil : (prefixes, remaining)
    }

    private static func stripSubtitle(_ text: String) -> (main: String, subtitle: String)? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        let patterns = [
            #"^(.+?)\s*[～〜]([^～〜]+)[～〜]\s*$"#,
            #"^(.+?)\s+[-–—―－~～〜]\s*(.+?)\s*$"#,
            #"^(.+?)\s*：\s*(.+?)\s*$"#,
            #"^(.+?):\s+(.+?)\s*$"#
        ]
        let trimSet = CharacterSet.whitespacesAndNewlines
            .union(CharacterSet(charactersIn: "-–—―－~～〜"))
        var earliestLocation = Int.max
        var earliestSplit: (main: String, subtitle: String)?
        for pattern in patterns {
            guard let regex = try? NSRegularExpression(pattern: pattern),
                  let match = regex.firstMatch(in: trimmed, range: NSRange(trimmed.startIndex..., in: trimmed)),
                  let mainRange = Range(match.range(at: 1), in: trimmed),
                  let subRange = Range(match.range(at: 2), in: trimmed)
            else { continue }
            let main = String(trimmed[mainRange]).trimmingCharacters(in: .whitespacesAndNewlines)
            let subtitle = String(trimmed[subRange]).trimmingCharacters(in: trimSet)
            guard !main.isEmpty, !subtitle.isEmpty else { continue }
            let splitLoc = match.range(at: 1).length
            if splitLoc < earliestLocation {
                earliestLocation = splitLoc
                earliestSplit = (main, subtitle)
            }
        }
        return earliestSplit
    }

    // MARK: - Cleaning Helpers

    private static func stripLeadingNoisePrefixes(_ text: String) -> String {
        var result = text
        while let regex = leadingPrefixRegex {
            let fullRange = NSRange(result.startIndex..., in: result)
            guard let match = regex.firstMatch(in: result, range: fullRange),
                  let content = firstCapturedGroup(in: result, match: match, indices: 1 ... 4),
                  isLeadingNoiseTag(content),
                  let matchRange = Range(match.range, in: result)
            else { break }
            let stripped = String(result[matchRange.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
            guard !stripped.isEmpty, stripped != result else { break }
            result = stripped
        }
        return result
    }

    private static func isLeadingNoiseTag(_ content: String) -> Bool {
        let folded = content.trimmingCharacters(in: .whitespacesAndNewlines).folding(
            options: [.caseInsensitive, .widthInsensitive],
            locale: Locale(identifier: "en_US_POSIX")
        )
        guard !folded.isEmpty else { return false }
        let range = NSRange(folded.startIndex..., in: folded)
        if leadingMarkerRegex?.firstMatch(in: folded, range: range) != nil {
            return true
        }
        return leadingTagTokensRegex?.firstMatch(in: folded, range: range) != nil
    }

    private static func stripTrailingNoiseSuffixes(_ text: String) -> String {
        var result = text
        while true {
            var next: String?
            if let regex = trailingBracketRegex {
                let fullRange = NSRange(result.startIndex..., in: result)
                if let match = regex.firstMatch(in: result, range: fullRange),
                   let prefixRange = Range(match.range(at: 1), in: result),
                   let content = firstCapturedGroup(in: result, match: match, indices: 2 ... 5),
                   isTrailingNoiseBracket(content) {
                    let candidate = String(result[prefixRange]).trimmingCharacters(in: .whitespacesAndNewlines)
                    if !candidate.isEmpty {
                        next = candidate
                    }
                }
            }
            if next == nil, let regex = trailingBareSuffixRegex {
                let fullRange = NSRange(result.startIndex..., in: result)
                if let match = regex.firstMatch(in: result, range: fullRange),
                   let matchRange = Range(match.range, in: result) {
                    let candidate = String(result[..<matchRange.lowerBound])
                        .trimmingCharacters(in: .whitespacesAndNewlines)
                    if !candidate.isEmpty {
                        next = candidate
                    }
                }
            }
            guard let updated = next, updated != result else {
                return result
            }
            result = updated
        }
    }

    private static func isTrailingNoiseBracket(_ content: String) -> Bool {
        let folded = content.trimmingCharacters(in: .whitespacesAndNewlines).folding(
            options: [.caseInsensitive, .widthInsensitive],
            locale: Locale(identifier: "en_US_POSIX")
        )
        guard !folded.isEmpty else { return false }
        let range = NSRange(folded.startIndex..., in: folded)
        if trailingBracketTokensRegex?.firstMatch(in: folded, range: range) != nil {
            return true
        }
        return trailingTranslationTagRegex?.firstMatch(in: folded, range: range) != nil
    }

    private static func firstCapturedGroup(
        in text: String,
        match: NSTextCheckingResult,
        indices: ClosedRange<Int>
    ) -> String? {
        for index in indices {
            let range = match.range(at: index)
            if range.location != NSNotFound, let swiftRange = Range(range, in: text) {
                return String(text[swiftRange])
            }
        }
        return nil
    }

    // MARK: - Character & Script Helpers

    private static func expandIterationMarks(_ text: String) -> String {
        var scalars: [UnicodeScalar] = []
        scalars.reserveCapacity(text.unicodeScalars.count)
        var previousIdeograph: UnicodeScalar?
        for scalar in text.unicodeScalars {
            if scalar.value == 0x3005 {
                if let previous = previousIdeograph {
                    scalars.append(previous)
                    continue
                }
            } else if (0x3400 ... 0x9FFF).contains(scalar.value) {
                previousIdeograph = scalar
            } else {
                previousIdeograph = nil
            }
            scalars.append(scalar)
        }
        return String(String.UnicodeScalarView(scalars))
    }

    private static func replaceUnicodeRomanNumerals(_ text: String) -> String {
        var result = ""
        result.reserveCapacity(text.count)
        for scalar in text.unicodeScalars {
            if (0x2160 ... 0x2169).contains(scalar.value) {
                result.append(String(scalar.value - 0x2160 + 1))
            } else if (0x2170 ... 0x2179).contains(scalar.value) {
                result.append(String(scalar.value - 0x2170 + 1))
            } else {
                result.unicodeScalars.append(scalar)
            }
        }
        return result
    }

    struct ScriptFlags {
        var hasHan = false
        var hasKana = false
        var hasLatin = false
    }

    static func scriptFlags(_ text: String) -> ScriptFlags {
        var flags = ScriptFlags()
        let folded = text.folding(options: .widthInsensitive, locale: Locale(identifier: "en_US_POSIX"))
        for scalar in folded.unicodeScalars {
            let value = scalar.value
            if (0x3400 ... 0x9FFF).contains(value) || (0x20000 ... 0x323AF).contains(value) {
                flags.hasHan = true
            } else if (0x3040 ... 0x30FF).contains(value), value != 0x30FB, value != 0x30FC {
                flags.hasKana = true
            } else if (0x0041 ... 0x005A).contains(value)
                || (0x0061 ... 0x007A).contains(value)
                || (0x00C0 ... 0x024F).contains(value) {
                flags.hasLatin = true
            }
        }
        return flags
    }

    private static func haveDifferentScripts(_ lhs: String, _ rhs: String) -> Bool {
        let left = scriptFlags(lhs)
        let right = scriptFlags(rhs)
        let leftHasScript = left.hasHan || left.hasKana || left.hasLatin
        let rightHasScript = right.hasHan || right.hasKana || right.hasLatin
        guard leftHasScript, rightHasScript else { return false }
        let leftCJK = left.hasHan || left.hasKana
        let rightCJK = right.hasHan || right.hasKana
        return left.hasKana != right.hasKana || leftCJK != rightCJK
    }

    private static func meetsScriptLength(_ normalizedText: String, minCJK: Int, minLatin: Int) -> Bool {
        let cjkCount = normalizedText.unicodeScalars.filter { (0x3040 ... 0x9FFF).contains($0.value) }.count
        if cjkCount > 0 {
            return cjkCount >= minCJK
        }
        return normalizedText.count >= minLatin
    }

    private static func developerMatches(_ brand: String, _ developer: String) -> Bool {
        let normBrand = normalized(brand)
        let normDev = normalized(developer)
        guard !normBrand.isEmpty, !normDev.isEmpty else { return false }
        return normDev.contains(normBrand) || normBrand.contains(normDev)
    }

    private static func studioMatches(_ brand: String, _ developer: String) -> Bool {
        let aliases = [
            "ゆずソフト": "yuzusoft", "アリスソフト": "alicesoft", "エウシュリー": "eushully",
            "まどそふと": "madosoft", "枕": "makura", "ケロq": "keroq", "ういんどみる": "windmill",
            "ラプラシアン": "laplacian", "nitro": "nitroplus"
        ]
        func canonical(_ text: String) -> String {
            var value = normalized(text)
            for suffix in ["corporation", "incorporated", "株式会社", "有限会社", "inc", "ltd", "llc"] {
                if value.hasSuffix(suffix), value.count > suffix.count {
                    value.removeLast(suffix.count)
                    break
                }
            }
            return aliases[value] ?? value
        }
        let separators = CharacterSet(charactersIn: ",，、/;&")
        let brands = Set(brand.components(separatedBy: separators).map(canonical).filter { !$0.isEmpty })
        let developers = Set(developer.components(separatedBy: separators).map(canonical).filter { !$0.isEmpty })
        return !brands.isDisjoint(with: developers)
    }

    private static func hasCompatibleDeveloper(query: String, game: GameMetadata) -> Bool {
        var remaining = query.trimmingCharacters(in: .whitespacesAndNewlines)
        while let regex = leadingPrefixRegex,
              let match = regex.firstMatch(in: remaining, range: NSRange(remaining.startIndex..., in: remaining)),
              let prefix = firstCapturedGroup(in: remaining, match: match, indices: 1 ... 4),
              let range = Range(match.range, in: remaining) {
            let folded = prefix.folding(
                options: [.caseInsensitive, .widthInsensitive],
                locale: Locale(identifier: "en_US_POSIX")
            )
            let prefixRange = NSRange(folded.startIndex..., in: folded)
            let isTranslation = translationMarkerRegex?.firstMatch(in: folded, range: prefixRange) != nil
            let isKnownTag = leadingTagTokensRegex?.firstMatch(in: folded, range: prefixRange) != nil
            if isKnownTag {
                for match in knownStudioTokenRegex?.matches(in: folded, range: prefixRange) ?? [] {
                    if let studioRange = Range(match.range, in: folded),
                       !studioMatches(String(folded[studioRange]), game.developer) {
                        return false
                    }
                }
            } else {
                let isStudio = knownStudioRegex?.firstMatch(in: folded, range: prefixRange) != nil
                    || studioMarkerRegex?.firstMatch(in: folded, range: prefixRange) != nil
                if isStudio, !isTranslation, !studioMatches(folded, game.developer) {
                    return false
                }
            }
            if !isLeadingNoiseTag(prefix), !developerMatches(prefix, game.developer),
               !titleVariants(for: game).contains(where: { titlesMatchExactly(self.query(query), $0) }) {
                return false
            }
            remaining = String(remaining[range.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return true
    }

    private static func chapterIdentity(_ title: String) -> String? {
        let folded = title.folding(options: .widthInsensitive, locale: Locale(identifier: "en_US_POSIX"))
        let subtitle = stripSubtitle(folded)?.subtitle
        let subject = subtitle ?? folded
        let regex = subtitle == nil ? explicitChapterRegex : chapterSubtitleRegex
        let range = NSRange(subject.startIndex..., in: subject)
        guard let match = regex?.firstMatch(in: subject, range: range),
              let swiftRange = Range(match.range, in: subject)
        else { return nil }
        return normalized(String(subject[swiftRange.lowerBound...]))
    }

    private static func chaptersCompatible(_ lhs: String, _ rhs: String) -> Bool {
        let left = chapterIdentity(lhs)
        let right = chapterIdentity(rhs)
        return left == right
    }

    private static func chapterBase(_ title: String) -> String? {
        let folded = title.folding(options: .widthInsensitive, locale: Locale(identifier: "en_US_POSIX"))
        if let split = stripSubtitle(folded), containsChapterSubtitleMarker(split.subtitle) {
            return split.main
        }
        let range = NSRange(folded.startIndex..., in: folded)
        guard let match = explicitChapterRegex?.firstMatch(in: folded, range: range),
              let swiftRange = Range(match.range, in: folded)
        else { return nil }
        return String(folded[..<swiftRange.lowerBound])
            .trimmingCharacters(in: .whitespacesAndNewlines.union(CharacterSet(charactersIn: "-–—―－~～〜:")))
    }

    private static func hasCompatibleChapter(query: String, game: GameMetadata) -> Bool {
        let officialTitles = [game.title, game.originalTitle].filter { !$0.isEmpty }
        let chapterTitles = officialTitles.filter { chapterIdentity($0) != nil }
        guard !chapterTitles.isEmpty || chapterIdentity(query) != nil else {
            return true
        }
        if chapterTitles.contains(where: { chaptersCompatible(query, $0) }) {
            return true
        }
        if chapterIdentity(query) != nil, !chapterTitles.isEmpty {
            return false
        }
        // A source can supply an independent translated chapter title as an alias.
        // A generic series title is shared by chapters and does not identify one.
        guard (game.alternateTitles ?? []).contains(where: { titlesMatchExactly(query, $0) }) else { return false }
        return !officialTitles.contains { title in
            if let base = chapterBase(title), titlesMatchExactly(query, base) {
                return true
            }
            if let base = chapterBase(query), titlesMatchExactly(title, base) {
                return true
            }
            return false
        }
    }

    private static func containsChapterSubtitleMarker(_ text: String) -> Bool {
        let folded = text.folding(
            options: [.caseInsensitive, .widthInsensitive],
            locale: Locale(identifier: "en_US_POSIX")
        )
        let range = NSRange(folded.startIndex..., in: folded)
        return chapterSubtitleRegex?.firstMatch(in: folded, range: range) != nil
    }

    private static func containsCJK(_ text: String) -> Bool {
        text.unicodeScalars.contains { (0x3040 ... 0x9FFF).contains($0.value) }
    }

    private static func edition(_ text: String) -> [String] {
        let folded = replaceUnicodeRomanNumerals(text).folding(
            options: [.caseInsensitive, .widthInsensitive],
            locale: Locale(identifier: "en_US_POSIX")
        )
        let pattern = #"\d+|(?<![a-z])(?:ii|iii|iv|vi|vii|viii|ix|fandisc|fan\s*disc|fd"#
            + #"|complete|remake|remaster(?:ed)?)(?![a-z])"#
            + #"|\+|ファンディスク|リメイク|リマスター|(?:重制|重製|重置)(?:版)?|(?:完全|完整)版"#
        guard let expression = try? NSRegularExpression(pattern: pattern) else { return [] }
        var seen = Set<String>()
        var tokens: [String] = []
        for match in expression.matches(in: folded, range: NSRange(folded.startIndex..., in: folded)) {
            guard let range = Range(match.range, in: folded) else { continue }
            let raw = String(folded[range])
            let collapsed = raw.replacingOccurrences(of: #"\s+"#, with: " ", options: .regularExpression)
            let canonical = editionTokenReplacements[collapsed] ?? raw
            if seen.insert(canonical).inserted {
                tokens.append(canonical)
            }
        }
        return tokens
    }
}
