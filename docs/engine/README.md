# Configured Engines

| Name                        | Category         | Implementation                                                                                     |
| --------------------------- | ---------------- | -------------------------------------------------------------------------------------------------- |
| 1x                          | images           | [`www1x`](../../vendor/searxng/searx/engines/www1x.py)                                             |
| adobe stock                 | images           | [`adobe_stock`](../../vendor/searxng/searx/engines/adobe_stock.py)                                 |
| adobe stock audio           | music            | [`adobe_stock`](../../vendor/searxng/searx/engines/adobe_stock.py)                                 |
| adobe stock video           | videos           | [`adobe_stock`](../../vendor/searxng/searx/engines/adobe_stock.py)                                 |
| annas archive               | files            | [`annas_archive`](../../vendor/searxng/searx/engines/annas_archive.py)                             |
| apple app store             | files            | [`apple_app_store`](../../vendor/searxng/searx/engines/apple_app_store.py)                         |
| apple maps                  | map              | [`apple_maps`](../../vendor/searxng/searx/engines/apple_maps.py)                                   |
| arch linux wiki             | it               | [`archlinux`](../../vendor/searxng/searx/engines/archlinux.py)                                     |
| arxiv                       | science          | [`arxiv`](../../vendor/searxng/searx/engines/arxiv.py)                                             |
| bing                        | general          | [`bing`](../../vendor/searxng/searx/engines/bing.py)                                               |
| bing images                 | images           | [`bing_images`](../../vendor/searxng/searx/engines/bing_images.py)                                 |
| bing news                   | news             | [`bing_news`](../../vendor/searxng/searx/engines/bing_news.py)                                     |
| bing videos                 | videos           | [`bing_videos`](../../vendor/searxng/searx/engines/bing_videos.py)                                 |
| bitbucket                   | it               | [`xpath`](../../vendor/searxng/searx/engines/xpath.py)                                             |
| brave                       | general          | [`brave`](../../vendor/searxng/searx/engines/brave.py)                                             |
| brave.images                | images           | [`brave`](../../vendor/searxng/searx/engines/brave.py)                                             |
| brave.news                  | news             | [`brave`](../../vendor/searxng/searx/engines/brave.py)                                             |
| brave.videos                | videos           | [`brave`](../../vendor/searxng/searx/engines/brave.py)                                             |
| crossref                    | science          | [`crossref`](../../vendor/searxng/searx/engines/crossref.py)                                       |
| ddg definitions             | general          | [`duckduckgo_definitions`](../../vendor/searxng/searx/engines/duckduckgo_definitions.py)           |
| docker hub                  | it               | [`docker_hub`](../../vendor/searxng/searx/engines/docker_hub.py)                                   |
| duckduckgo                  | general          | [`duckduckgo`](../../vendor/searxng/searx/engines/duckduckgo.py)                                   |
| duckduckgo images           | images           | [`duckduckgo_extra`](../../vendor/searxng/searx/engines/duckduckgo_extra.py)                       |
| duckduckgo news             | news             | [`duckduckgo_extra`](../../vendor/searxng/searx/engines/duckduckgo_extra.py)                       |
| duckduckgo videos           | videos           | [`duckduckgo_extra`](../../vendor/searxng/searx/engines/duckduckgo_extra.py)                       |
| github                      | it               | [`github`](../../vendor/searxng/searx/engines/github.py)                                           |
| gitlab                      | it               | [`gitlab`](../../vendor/searxng/searx/engines/gitlab.py)                                           |
| google                      | general          | [`google`](../../vendor/searxng/searx/engines/google.py)                                           |
| google images               | images           | [`google_images`](../../vendor/searxng/searx/engines/google_images.py)                             |
| google news                 | news             | [`google_news`](../../vendor/searxng/searx/engines/google_news.py)                                 |
| google play apps            | files            | [`google_play`](../../vendor/searxng/searx/engines/google_play.py)                                 |
| google play movies          | videos           | [`google_play`](../../vendor/searxng/searx/engines/google_play.py)                                 |
| google scholar              | science          | [`google_scholar`](../../vendor/searxng/searx/engines/google_scholar.py)                           |
| google videos               | videos           | [`google_videos`](../../vendor/searxng/searx/engines/google_videos.py)                             |
| hackernews                  | it               | [`hackernews`](../../vendor/searxng/searx/engines/hackernews.py)                                   |
| huggingface                 | it               | [`huggingface`](../../vendor/searxng/searx/engines/huggingface.py)                                 |
| huggingface datasets        | it               | [`huggingface`](../../vendor/searxng/searx/engines/huggingface.py)                                 |
| huggingface spaces          | it               | [`huggingface`](../../vendor/searxng/searx/engines/huggingface.py)                                 |
| library genesis             | files            | [`xpath`](../../vendor/searxng/searx/engines/xpath.py)                                             |
| lobste.rs                   | it               | [`xpath`](../../vendor/searxng/searx/engines/xpath.py)                                             |
| lucide                      | images           | [`lucide`](../../vendor/searxng/searx/engines/lucide.py)                                           |
| material icons              | images           | [`material_icons`](../../vendor/searxng/searx/engines/material_icons.py)                           |
| mdn                         | it               | [`json_engine`](../../vendor/searxng/searx/engines/json_engine.py)                                 |
| openairedatasets            | science          | [`json_engine`](../../vendor/searxng/searx/engines/json_engine.py)                                 |
| openairepublications        | science          | [`json_engine`](../../vendor/searxng/searx/engines/json_engine.py)                                 |
| openalex                    | science          | [`openalex`](../../vendor/searxng/searx/engines/openalex.py)                                       |
| openlibrary                 | books            | [`openlibrary`](../../vendor/searxng/searx/engines/openlibrary.py)                                 |
| openstreetmap               | map              | [`openstreetmap`](../../vendor/searxng/searx/engines/openstreetmap.py)                             |
| photon                      | map              | [`photon`](../../vendor/searxng/searx/engines/photon.py)                                           |
| pixabay images              | images           | [`pixabay`](../../vendor/searxng/searx/engines/pixabay.py)                                         |
| pixabay videos              | videos           | [`pixabay`](../../vendor/searxng/searx/engines/pixabay.py)                                         |
| public domain image archive | images           | [`public_domain_image_archive`](../../vendor/searxng/searx/engines/public_domain_image_archive.py) |
| pubmed                      | science          | [`pubmed`](../../vendor/searxng/searx/engines/pubmed.py)                                           |
| soundcloud                  | music            | [`soundcloud`](../../vendor/searxng/searx/engines/soundcloud.py)                                   |
| sourcehut                   | it               | [`sourcehut`](../../vendor/searxng/searx/engines/sourcehut.py)                                     |
| stackoverflow               | it               | [`stackexchange`](../../vendor/searxng/searx/engines/stackexchange.py)                             |
| startpage                   | general          | [`startpage`](../../vendor/searxng/searx/engines/startpage.py)                                     |
| startpage images            | images           | [`startpage`](../../vendor/searxng/searx/engines/startpage.py)                                     |
| startpage news              | news             | [`startpage`](../../vendor/searxng/searx/engines/startpage.py)                                     |
| unsplash                    | images           | [`unsplash`](../../vendor/searxng/searx/engines/unsplash.py)                                       |
| vimeo                       | videos           | [`vimeo`](../../vendor/searxng/searx/engines/vimeo.py)                                             |
| wikibooks                   | books            | [`mediawiki`](../../vendor/searxng/searx/engines/mediawiki.py)                                     |
| wikicommons.audio           | music            | [`wikicommons`](../../vendor/searxng/searx/engines/wikicommons.py)                                 |
| wikicommons.files           | files            | [`wikicommons`](../../vendor/searxng/searx/engines/wikicommons.py)                                 |
| wikicommons.images          | images           | [`wikicommons`](../../vendor/searxng/searx/engines/wikicommons.py)                                 |
| wikicommons.videos          | videos           | [`wikicommons`](../../vendor/searxng/searx/engines/wikicommons.py)                                 |
| wikidata                    | general          | [`wikidata`](../../vendor/searxng/searx/engines/wikidata.py)                                       |
| wikinews                    | news             | [`mediawiki`](../../vendor/searxng/searx/engines/mediawiki.py)                                     |
| wikipedia                   | general          | [`wikipedia`](../../vendor/searxng/searx/engines/wikipedia.py)                                     |
| wikiquote                   | general          | [`mediawiki`](../../vendor/searxng/searx/engines/mediawiki.py)                                     |
| wikisource                  | general          | [`mediawiki`](../../vendor/searxng/searx/engines/mediawiki.py)                                     |
| wikispecies                 | general, science | [`mediawiki`](../../vendor/searxng/searx/engines/mediawiki.py)                                     |
| wikiversity                 | general          | [`mediawiki`](../../vendor/searxng/searx/engines/mediawiki.py)                                     |
| wikivoyage                  | general          | [`mediawiki`](../../vendor/searxng/searx/engines/mediawiki.py)                                     |
| wolframalpha                | general          | [`wolframalpha_noapi`](../../vendor/searxng/searx/engines/wolframalpha_noapi.py)                   |
| yahoo                       | general          | [`yahoo`](../../vendor/searxng/searx/engines/yahoo.py)                                             |
| yahoo news                  | news             | [`yahoo_news`](../../vendor/searxng/searx/engines/yahoo_news.py)                                   |
| yandex                      | general          | [`yandex`](../../vendor/searxng/searx/engines/yandex.py)                                           |
| yandex images               | images           | [`yandex`](../../vendor/searxng/searx/engines/yandex.py)                                           |
| yandex music                | music            | [`yandex_music`](../../vendor/searxng/searx/engines/yandex_music.py)                               |
