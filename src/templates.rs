use tera::{Context,Tera};
use serde::Serialize;

pub const GOPHER_MAP: &str = r#"
 .----------------.
| .--------------. |
| |   _____      | |
| |  |_   _|     | |
| |    | |       | |
| |    | |   _   | |
| |   _| |__/ |  | |
| |  |________|  | |
| |              | |
| '--------------' |
 '----------------'

This is an unofficial Lobste.rs mirror on gopher.
You can find the 25 hottest stories and their comments.
Sync happens every 10 minutes or so.

Last updated {{ now() | date(format="%a %b %e %T %Y") }}

{% for story in stories %}
h[{{ story.score }}] - {{ story.title }}{% if story.url %}  URL:{{ story.url }}{% endif %}
Submitted {{ story.date | date(format="%a %b %e %T %Y") }} by {{ story.user.name }} | {{ story.tags | join(sep=", ") }}
0View comments ({{ story.count }}) {{ story.id }}.txt
{% endfor %}
"#;

pub const GOPHER_PAGE: &str = r#"
 .----------------.
| .--------------. |
| |   _____      | |
| |  |_   _|     | |
| |    | |       | |
| |    | |   _   | |
| |   _| |__/ |  | |
| |  |________|  | |
| |              | |
| '--------------' |
 '----------------'


Viewing comments for "{{ story.title }}"
---
{% for comment in story.comments %}
{{ comment.user.name }} commented [{{ comment.score }}]
{{ comment.text }}{#| cleanup(comment.indentation) }}TODO#}
{% endfor %}
"#;

pub fn build_template<T: Serialize + ?Sized>(name: &str, context: Vec<(&str, &T)>, tera: &Tera) -> Result<String, tera::Error> {
    let mut c = Context::new();
    for (k, v) in context {
        c.insert(k, v);
    }
    tera.render(name, &c)
}
