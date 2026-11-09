import Foundation

// Exact name tables from icu.rs. Unsupported locales deliberately fall back to English.
struct ICUNames: Sendable {
    var months: [String]
    var months_abbr: [String]
    var weekdays: [String]
    var weekdays_abbr: [String]
    var eras: [String]
    var eras_full: [String]
    var am: String
    var pm: String
}

enum ICUNameTables {
    static let en = ICUNames(
        months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
        months_abbr: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
        weekdays: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
        weekdays_abbr: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
        eras: ["BC", "AD"],
        eras_full: ["Before Christ", "Anno Domini"],
        am: "AM", pm: "PM")
    static let fr = ICUNames(
        months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
        months_abbr: ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
        weekdays: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
        weekdays_abbr: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
        eras: ["av. J.-C.", "ap. J.-C."],
        eras_full: ["avant Jésus-Christ", "après Jésus-Christ"],
        am: "AM", pm: "PM")
    static let de = ICUNames(
        months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
        months_abbr: ["Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.", "Nov.", "Dez."],
        weekdays: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
        weekdays_abbr: ["So.", "Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa."],
        eras: ["v. Chr.", "n. Chr."],
        eras_full: ["v. Chr.", "n. Chr."],
        am: "AM", pm: "PM")
    static let es = ICUNames(
        months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
        months_abbr: ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic"],
        weekdays: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
        weekdays_abbr: ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"],
        eras: ["a. C.", "d. C."],
        eras_full: ["antes de Cristo", "después de Cristo"],
        am: "a. m.", pm: "p. m.")
    static let it = ICUNames(
        months: ["gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno", "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre"],
        months_abbr: ["gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic"],
        weekdays: ["domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"],
        weekdays_abbr: ["dom", "lun", "mar", "mer", "gio", "ven", "sab"],
        eras: ["a.C.", "d.C."],
        eras_full: ["avanti Cristo", "dopo Cristo"],
        am: "AM", pm: "PM")
    static let pt = ICUNames(
        months: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
        months_abbr: ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."],
        weekdays: ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
        weekdays_abbr: ["dom.", "seg.", "ter.", "qua.", "qui.", "sex.", "sáb."],
        eras: ["a.C.", "d.C."],
        eras_full: ["antes de Cristo", "depois de Cristo"],
        am: "AM", pm: "PM")
    static let nl = ICUNames(
        months: ["januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september", "oktober", "november", "december"],
        months_abbr: ["jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec"],
        weekdays: ["zondag", "maandag", "dinsdag", "woensdag", "donderdag", "vrijdag", "zaterdag"],
        weekdays_abbr: ["zo", "ma", "di", "wo", "do", "vr", "za"],
        eras: ["v.Chr.", "n.Chr."],
        eras_full: ["voor Christus", "na Christus"],
        am: "a.m.", pm: "p.m.")
    static let ar = ICUNames(
        months: ["يناير", "فبراير", "مارس", "أبريل", "مايو", "يونيو", "يوليو", "أغسطس", "سبتمبر", "أكتوبر", "نوفمبر", "ديسمبر"],
        months_abbr: ["يناير", "فبراير", "مارس", "أبريل", "مايو", "يونيو", "يوليو", "أغسطس", "سبتمبر", "أكتوبر", "نوفمبر", "ديسمبر"],
        weekdays: ["الأحد", "الاثنين", "الثلاثاء", "الأربعاء", "الخميس", "الجمعة", "السبت"],
        weekdays_abbr: ["الأحد", "الاثنين", "الثلاثاء", "الأربعاء", "الخميس", "الجمعة", "السبت"],
        eras: ["ق.م", "م"],
        eras_full: ["قبل الميلاد", "ميلادي"],
        am: "ص", pm: "م")
}
