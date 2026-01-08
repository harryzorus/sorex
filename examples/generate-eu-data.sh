#!/bin/bash
# Generate European Countries example dataset for Sorex CLI testing
# Contains realistic Wikipedia-style content for each country

DIR="$(dirname "$0")/eu-countries"
mkdir -p "$DIR"

# Generate manifest
cat > "$DIR/manifest.json" << 'EOF'
{
  "version": 1,
  "documents": [
    "0.json", "1.json", "2.json", "3.json", "4.json",
    "5.json", "6.json", "7.json", "8.json", "9.json",
    "10.json", "11.json", "12.json", "13.json", "14.json",
    "15.json", "16.json", "17.json", "18.json", "19.json",
    "20.json", "21.json", "22.json", "23.json", "24.json",
    "25.json", "26.json", "27.json", "28.json", "29.json"
  ],
  "indexes": {
    "all": {
      "include": "*"
    },
    "nordic": {
      "include": { "category": "nordic" }
    },
    "mediterranean": {
      "include": { "category": "mediterranean" }
    }
  }
}
EOF

echo "Generating country data files..."

# Austria (0)
cat > "$DIR/0.json" << 'EOFCOUNTRY'
{
  "id": 0,
  "slug": "austria",
  "title": "Austria",
  "excerpt": "A landlocked country in Central Europe known for its Alpine scenery and musical heritage",
  "href": "/countries/austria",
  "type": "page",
  "category": "central",
  "text": "Austria, officially the Republic of Austria, is a landlocked country in the southern part of Central Europe, lying in the Eastern Alps. It is a federation of nine states, one of which is the capital, Vienna, the most populous city and state. Austria is bordered by Germany to the northwest, the Czech Republic to the north, Slovakia to the northeast, Hungary to the east, Slovenia and Italy to the south, and Switzerland and Liechtenstein to the west. The country occupies an area of 83,879 square kilometers and has a population of around 9 million people.\n\nAustria emerged from the remnants of the Eastern Frankish and Hungarian kingdoms, and was ruled by the House of Habsburg for centuries. It was a major power in Europe, particularly through the Austro-Hungarian Empire, which was one of the great powers until its defeat in World War I. The First Austrian Republic was established in 1918. After annexation by Nazi Germany in 1938 and subsequent occupation by the Allies, Austria's sovereignty was restored in 1955 with the Austrian State Treaty.\n\nThe terrain of Austria is highly mountainous, lying within the Alps. Only 32 percent of the country is below 500 meters, and its highest point is 3,798 meters. The majority of the population speaks local Bavarian dialects of German as their native language, and German in its standard form is the country's official language. Other local official languages are Hungarian, Burgenland Croatian, and Slovene.\n\nAustria consistently ranks high in quality of life and, as of 2018, was ranked 15th in the world for its Human Development Index. Vienna is regularly voted one of the most livable cities in the world. The country has developed a high standard of living and in 2020 had a nominal per capita GDP of over $48,000. Austria is a member of the United Nations since 1955 and joined the European Union in 1995. It hosts the headquarters of the OSCE and OPEC and is a founding member of the OECD.\n\nAustrian culture has been enormously influential in the fields of music, philosophy, and science. Vienna was a leading center of musical innovation in the late 18th and early 19th centuries. Composers such as Wolfgang Amadeus Mozart, Joseph Haydn, Ludwig van Beethoven, Franz Schubert, and Johann Strauss II were either born in Austria or made their careers there. The Vienna Philharmonic Orchestra is considered one of the finest orchestras in the world. Austrian cuisine derives from the cuisine of the Austro-Hungarian Empire. Austrian dishes include Wiener Schnitzel, Tafelspitz, Apfelstrudel, and Sachertorte.\n\nThe economy of Austria is a well-developed social market economy, with a skilled labour force and a high standard of living. Austria is one of the richest countries in the world. Until the 1980s, many of Austria's largest industry firms were nationalised. In recent years, privatisation has reduced state holdings to a level comparable to other European economies. Labour movements are particularly influential, exercising large influence on labour politics. Tourism is an important part of the economy, accounting for almost 9 percent of Austrian gross domestic product.\n\nAustria has a temperate, alpine climate. There are four seasons with typical temperature variation. The winters are cold and the summers are warm. Precipitation is spread fairly evenly throughout the year. The Alps also act as a climate divide, as weather conditions differ between the north and south of the mountain range. The Danube valley, the Wienerwald, and the eastern lowlands have a continental climate with less rain than the alpine areas.",
  "fieldBoundaries": [
    {"docId": 0, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 0, "start": 8, "end": 3500, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Austria"

# Belgium (1)
cat > "$DIR/1.json" << 'EOFCOUNTRY'
{
  "id": 1,
  "slug": "belgium",
  "title": "Belgium",
  "excerpt": "A federal monarchy in Western Europe known as the de facto capital of the European Union",
  "href": "/countries/belgium",
  "type": "page",
  "category": "western",
  "text": "Belgium, officially the Kingdom of Belgium, is a country in Northwestern Europe. The country is bordered by the Netherlands to the north, Germany to the east, Luxembourg to the southeast, France to the southwest, and the North Sea to the northwest. It covers an area of 30,689 square kilometers and has a population of more than 11.5 million, making it the 22nd most densely populated country in the world and the 6th most densely populated country in Europe.\n\nThe capital and largest city is Brussels; other major cities are Antwerp, Ghent, Charleroi, Liège, Bruges, Namur, and Leuven. Belgium is a sovereign state and a federal constitutional monarchy with a parliamentary system. Its institutional organization is complex and is structured on both regional and linguistic grounds. It is divided into three highly autonomous regions: Flanders in the north, Wallonia in the south, and the Brussels-Capital Region.\n\nBelgium is home to two main linguistic groups: the Dutch-speaking Flemish community, which constitutes about 60 percent of the population, and the French-speaking community, which constitutes about 40 percent. A small German-speaking community of around one percent also exists in the Eastern Cantons. The Brussels-Capital Region is officially bilingual in French and Dutch, although French is the dominant language.\n\nHistorically, Belgium, the Netherlands, and Luxembourg were known as the Low Countries, which used to cover a somewhat larger area than the current Benelux group of states. From the end of the Middle Ages until the 17th century, the area of Belgium was a prosperous and cosmopolitan center of commerce and culture. From the 16th century until the Belgian Revolution in 1830, many battles between European powers were fought in the area of Belgium, causing it to be dubbed the battleground of Europe.\n\nBelgium is part of an area known as the Blue Banana, indicating a high population density. Since its independence, Belgium has participated in the Industrial Revolution and during the course of the 20th century, possessed a number of colonies in Africa. The second half of the 20th century was marked by rising tensions between the Dutch-speaking and the French-speaking citizens fueled by differences in language and culture and the unequal economic development of Flanders and Wallonia.\n\nThe economy of Belgium is a modern, open, and private-enterprise-based economy that has capitalized on its central geographic location, highly developed transport network, and diversified industrial and commercial base. Industry is concentrated mainly in the populous Flanders region in the north. With few natural resources, Belgium imports substantial quantities of raw materials and exports a large volume of manufactures, making its economy vulnerable to shifts in world trade.\n\nBelgium is famous for its chocolate, waffles, beer, and French fries. Belgian cuisine is noted for its high quality and diversity. Many highly ranked restaurants can be found in the various gastronomic guides. Belgium is also one of the largest chocolate producers and exporters in the world. Belgian beer is also highly regarded globally, with a wide variety of styles including Trappist ales, lambics, and specialty beers.\n\nBelgium has three official languages: Dutch, French, and German. As of 2024, the population of Belgium is around 11.7 million. The country has a high standard of living, with one of the highest GDPs per capita in Europe. Belgium is a founding member of the European Union and hosts the headquarters of many major EU institutions, including the European Commission, the Council of the European Union, and the European Council, as well as one of two seats of the European Parliament.",
  "fieldBoundaries": [
    {"docId": 1, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 1, "start": 8, "end": 3800, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Belgium"

# Bulgaria (2)
cat > "$DIR/2.json" << 'EOFCOUNTRY'
{
  "id": 2,
  "slug": "bulgaria",
  "title": "Bulgaria",
  "excerpt": "A country on the Balkan Peninsula with rich ancient history and Black Sea coastline",
  "href": "/countries/bulgaria",
  "type": "page",
  "category": "eastern",
  "text": "Bulgaria, officially the Republic of Bulgaria, is a country in Southeast Europe. It is situated on the eastern flank of the Balkans, and is bordered by Romania to the north, Serbia and North Macedonia to the west, Greece and Turkey to the south, and the Black Sea to the east. Bulgaria covers a territory of 110,994 square kilometers, and is the sixteenth-largest country in Europe. Sofia is the nation's capital and largest city; other major cities are Plovdiv, Varna, and Burgas.\n\nOne of the earliest societies in the lands of modern-day Bulgaria was the Neolithic Karanovo culture. In the 6th to 3rd century BC, the region was a battleground for ancient Thracians, Persians, Celts and Macedonians. Stability came when the Roman Empire conquered the region in AD 45. After the Roman state fragmented, tribal invasions in the region resumed. Around the 6th century, these territories were settled by the early Slavs.\n\nThe First Bulgarian Empire was established in 681, and became the first Slavic state to adopt Christianity as its state religion in 864. It adopted the Cyrillic script, which was developed in the Preslav Literary School, and became one of the cultural and literary centers of medieval Europe. The medieval Bulgarian state survived until 1018, when it was conquered by the Byzantine Empire. A successful Bulgarian revolt in 1185 established a Second Bulgarian Empire, which reached its apex under Ivan Asen II.\n\nAfter numerous exhausting wars and feudal strife, the Second Bulgarian Empire disintegrated in 1396 and its territories fell under Ottoman rule for nearly five centuries. The Russo-Turkish War of 1877–78 led to the formation of the Third Bulgarian State. Many ethnic Bulgarians were left outside the borders of the new nation, which led to several conflicts with its neighbors and an alliance with Germany in both world wars.\n\nIn 1946, Bulgaria became a one-party socialist state as part of the Soviet-led Eastern Bloc. The ruling Communist Party gave up its monopoly on power after the revolutions of 1989 and allowed multiparty elections. Bulgaria then transitioned into a democracy and a market-based economy. Since adopting a democratic constitution in 1991, Bulgaria has been a unitary parliamentary republic with a high degree of political, administrative, and economic centralization.\n\nBulgaria is a developing country, with an upper-middle income economy, ranking 56th in the Human Development Index. Its market economy is part of the European Single Market and is largely based on services, followed by industry—especially machine building and mining—and agriculture. Widespread corruption is a major socioeconomic issue. Bulgaria is a member of the European Union, NATO, and the Council of Europe; it is also a founding member of the OSCE, and has taken a seat on the UN Security Council three times.\n\nThe climate of Bulgaria is diverse. The northern part has a continental climate with cold winters and hot summers. The Black Sea coast has a more moderate climate, while the south has a Mediterranean climate with mild winters and dry summers. The country has significant biodiversity, with 93 mammal species, 404 bird species, and 36 reptile species.\n\nBulgarian culture has been shaped by the country's location at the crossroads of various civilizations. It has made contributions to global culture in the spheres of art, music, literature, and science. Traditional Bulgarian music uses distinctive vocal techniques and instruments such as the gaida (bagpipe) and kaval (flute). Bulgarian folk dances, like the horo, are an important part of the national heritage.",
  "fieldBoundaries": [
    {"docId": 2, "start": 0, "end": 8, "fieldType": "title", "sectionId": null},
    {"docId": 2, "start": 9, "end": 3600, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Bulgaria"

# Croatia (3)
cat > "$DIR/3.json" << 'EOFCOUNTRY'
{
  "id": 3,
  "slug": "croatia",
  "title": "Croatia",
  "excerpt": "A Mediterranean country famous for its Adriatic coastline and historic Dubrovnik",
  "href": "/countries/croatia",
  "type": "page",
  "category": "mediterranean",
  "text": "Croatia, officially the Republic of Croatia, is a country at the crossroads of Central and Southeast Europe. It borders Slovenia to the northwest, Hungary to the northeast, Serbia to the east, Bosnia and Herzegovina and Montenegro to the southeast, and shares a maritime border with Italy to the west and southwest. Croatia's capital and largest city is Zagreb. The country spans 56,594 square kilometers and has a population of around 3.9 million.\n\nThe Croats arrived in the area of present-day Croatia during the early part of the 7th century AD. They organized the state into two duchies by the 9th century. Croatia was first internationally recognized as an independent state on 7 June 879 during the reign of Duke Branimir. Tomislav became the first king by 925, elevating Croatia to the status of a kingdom. The Kingdom of Croatia retained its sovereignty for almost two centuries, reaching its peak during the rule of Kings Peter Krešimir IV and Dmitar Zvonimir.\n\nCroatia entered a personal union with Hungary in 1102. In 1527, faced with Ottoman conquest, the Croatian Parliament elected Ferdinand I of Austria as the ruler of Croatia. After World War I, Croatia was included in the unrecognized State of Slovenes, Croats, and Serbs which merged into the Kingdom of Yugoslavia. A fascist puppet state known as the Independent State of Croatia, backed by Fascist Italy and Nazi Germany, existed during World War II.\n\nAfter World War II, Croatia became a founding member and a federal constituent of the Socialist Federal Republic of Yugoslavia, a constitutionally socialist state. On 25 June 1991, Croatia declared independence, which came into effect on 8 October of the same year. The Croatian War of Independence was fought successfully during the four years following the declaration.\n\nCroatia is classified by the World Bank as a high-income economy and is a member of the European Union, the United Nations, the Council of Europe, NATO, the World Trade Organization, and a founding member of the Union for the Mediterranean. Croatia joined the Schengen Area and the Eurozone in January 2023. An active participant in United Nations peacekeeping, Croatia has contributed troops to the NATO-led mission in Afghanistan.\n\nTourism is a significant source of revenue, with Croatia ranked among the top 20 most popular tourist destinations in the world. The country's Adriatic Sea coast, with its 1,244 islands, is one of the most indented coastlines in the Mediterranean. Eight of Croatia's national parks and two nature parks are major attractions. Dubrovnik, Split, and Zadar are popular historic cities.\n\nThe Croatian economy is a developed high-income service-based economy with the tertiary sector accounting for 70 percent of total gross domestic product. The industrial sector accounts for 26 percent of GDP. Agriculture accounts for 3.7 percent of GDP and employs 2.3 percent of the workforce. Industrial output is concentrated in shipbuilding, food processing, pharmaceuticals, information technology, biochemistry, and timber.\n\nCroatia has an educational system that spans from pre-school to tertiary level. Primary education is compulsory for children from ages 6 to 15. Croatian culture is based on a long history dating back to the 7th century. The country is home to eight UNESCO World Heritage Sites. Croatia has produced many notable scientists, artists, writers, and athletes who have contributed to world culture and knowledge.",
  "fieldBoundaries": [
    {"docId": 3, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 3, "start": 8, "end": 3400, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Croatia"

# Cyprus (4)
cat > "$DIR/4.json" << 'EOFCOUNTRY'
{
  "id": 4,
  "slug": "cyprus",
  "title": "Cyprus",
  "excerpt": "An island nation in the Eastern Mediterranean with ancient Greek and Turkish heritage",
  "href": "/countries/cyprus",
  "type": "page",
  "category": "mediterranean",
  "text": "Cyprus, officially the Republic of Cyprus, is an island country in the Eastern Mediterranean. It is the third largest and third most populous island in the Mediterranean, and is situated south of Turkey, west of Syria and Lebanon, north of Egypt, east of Greece, and southeast of Malta. The country's capital and largest city is Nicosia. Cyprus has a population of approximately 1.2 million people.\n\nThe earliest confirmed site of human activity on Cyprus dates from around 10,000 BC. Archaeological remains from this period include the well-preserved Neolithic village of Khirokitia, a UNESCO World Heritage Site. Cyprus was settled by Mycenaean Greeks in two waves in the 2nd millennium BC. As a strategic location in the Eastern Mediterranean, it was subsequently occupied by several major powers, including the empires of the Assyrians, Egyptians and Persians, from whom the island was seized in 333 BC by Alexander the Great.\n\nSubsequent rule by Ptolemaic Egypt, the Classical and Eastern Roman Empire, Arab caliphates for a short period, the French Lusignan dynasty and the Venetians, was followed by over three centuries of Ottoman rule between 1571 and 1878. Cyprus was placed under the United Kingdom's administration based on the Cyprus Convention in 1878 and was formally annexed by the UK in 1914. The future of the island became a matter of disagreement between the two prominent ethnic communities, Greek Cypriots, who with their Greek Orthodox church had long desired union with Greece, and Turkish Cypriots, who favored either the island remaining under British rule or partition.\n\nFollowing nationalist violence in the 1950s, Cyprus was granted independence in 1960 on the basis of the Zürich and London Agreement. The island has been divided since 1974, when Turkey invaded and occupied the northern portion following a Greek-sponsored coup. The Turkish Cypriot community declared independence unilaterally in 1983 as the Turkish Republic of Northern Cyprus, but this entity is recognized only by Turkey. The entire island, including the areas under Turkish occupation, entered the European Union in 2004.\n\nCyprus is a major tourist destination in the Mediterranean. The economy, which is classified as advanced and high-income, has been diversified over recent decades. Besides tourism, the economy relies on financial services, shipping and real estate. Cyprus is a member of the United Nations along with most of its agencies and many other international organizations. It has been a member of the Commonwealth since 1961 and was a founding member of the Non-Aligned Movement until joining the EU.\n\nThe climate of Cyprus is subtropical Mediterranean and semi-arid with very mild winters and warm to hot summers. Snow is possible only in the Troodos Mountains in the central part of the island. Rain occurs mainly in winter, with summer being generally dry. Cyprus has the warmest climate in the Mediterranean part of the European Union.\n\nCypriot culture is divided between the two main ethnic communities. Greek Cypriot culture is closely related to that of mainland Greece and the wider Greek world. The island has a rich tradition of folk music, dance, and poetry. Traditional Cypriot cuisine includes dishes like souvlaki, halloumi cheese, and various meze. The island is also known for its ancient archaeological sites, including the Tombs of the Kings in Paphos and the ancient city of Kourion.",
  "fieldBoundaries": [
    {"docId": 4, "start": 0, "end": 6, "fieldType": "title", "sectionId": null},
    {"docId": 4, "start": 7, "end": 3400, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Cyprus"

# Czech Republic (5)
cat > "$DIR/5.json" << 'EOFCOUNTRY'
{
  "id": 5,
  "slug": "czech-republic",
  "title": "Czech Republic",
  "excerpt": "A Central European country known for Prague's medieval architecture and rich beer culture",
  "href": "/countries/czech-republic",
  "type": "page",
  "category": "central",
  "text": "The Czech Republic, also known by its short-form name Czechia, is a landlocked country in Central Europe. Historically known as Bohemia, it is bordered by Austria to the south, Germany to the west, Poland to the northeast, and Slovakia to the southeast. The Czech Republic has a hilly landscape covering an area of 78,871 square kilometers with a mostly temperate continental and oceanic climate. The capital and largest city is Prague; other major cities and urban areas include Brno, Ostrava, Plzeň, and Liberec.\n\nThe Duchy of Bohemia was founded in the late 9th century under Great Moravia. It was formally recognized as an Imperial State of the Holy Roman Empire in 1002 and became a kingdom in 1198. Following the Battle of Mohács in 1526, the whole Crown of Bohemia was gradually integrated into the Habsburg monarchy. The Protestant Bohemian Revolt led to the Thirty Years' War. After the Battle of White Mountain, the Habsburgs consolidated their rule. With the dissolution of the Holy Roman Empire in 1806, the Crown lands became part of the Austrian Empire.\n\nIn the 19th century, the Czech lands became more industrialized and in 1918, most of the Czech lands became part of the First Czechoslovak Republic following the collapse of Austria-Hungary after World War I. After Munich Agreement in 1938, Nazi Germany systematically took control over the Czech lands. Czechoslovakia was restored in 1945 and three years later became an Eastern Bloc communist state following a coup d'état in 1948.\n\nAttempts to liberalize the government and economy were suppressed by a Soviet-led invasion of the country during the Prague Spring in 1968. In November 1989, the Velvet Revolution ended communist rule in the country and restored democracy. On 1 January 1993, Czechoslovakia peacefully dissolved into its constituent states, the Czech Republic and Slovakia.\n\nThe Czech Republic is a developed country with an advanced, high-income export-oriented social market economy. It is a welfare state with a European social model, universal health care, and tuition-free university education. It ranks 32nd in the Human Development Index. The Czech Republic is a member of NATO, the European Union, the OECD, the OSCE, the Council of Europe, and the Visegrád Group.\n\nThe Czech Republic has a rich cultural heritage. It is the home of many cultural contributions including Czech literature, music, cinema, and art. Prague is one of the most visited cities in Europe and contains one of the world's most pristine and varied collections of architecture, from Art Nouveau to Baroque, Renaissance, Cubist, Gothic, Neo-Classical and ultra-modern. Historic Prague Castle is the largest ancient castle in the world according to Guinness World Records.\n\nCzech cuisine features dishes such as svíčková (marinated sirloin), knedlíky (dumplings), and trdelník (sweet pastry). The country is known for its beer culture and has the highest beer consumption per capita in the world. Czech beer traditions include famous brands like Pilsner Urquell, Budweiser Budvar, and Staropramen.\n\nThe economy of the Czech Republic is an industrialized, developed, high-income, export-oriented social market economy based on services, manufacturing, and innovation. Major industries include motor vehicles, machinery, electrical and electronic equipment, and steel production. Tourism is also a major contributor to the economy, with Prague being one of the most visited cities in Europe.",
  "fieldBoundaries": [
    {"docId": 5, "start": 0, "end": 14, "fieldType": "title", "sectionId": null},
    {"docId": 5, "start": 15, "end": 3600, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Czech Republic"

# Denmark (6)
cat > "$DIR/6.json" << 'EOFCOUNTRY'
{
  "id": 6,
  "slug": "denmark",
  "title": "Denmark",
  "excerpt": "A Nordic country known for its design heritage, cycling culture, and hygge lifestyle",
  "href": "/countries/denmark",
  "type": "page",
  "category": "nordic",
  "text": "Denmark, officially the Kingdom of Denmark, is a Nordic country in Northern Europe. It is the most southern of the Scandinavian nations and is southwest of Sweden, south of Norway, and bordered to the south by Germany. The Kingdom of Denmark is constitutionally a unitary state comprising Denmark proper and the two autonomous territories in the North Atlantic Ocean: the Faroe Islands and Greenland. Denmark proper is the southernmost of the Scandinavian countries, lying southwest of Sweden and south of Norway, and bordered to the south by Germany.\n\nDenmark proper consists of a peninsula, Jutland, and an archipelago of 443 named islands, of which the largest are Zealand, Funen, and the North Jutlandic Island. The islands are characterized by flat, arable land and sandy coasts, low elevation and a temperate climate. Denmark has a total area of 42,943 square kilometers and a population of approximately 5.9 million, of which around 800,000 live in the capital and largest city, Copenhagen. The Faroe Islands and Greenland have populations of approximately 50,000 and 57,000 respectively.\n\nThe unified kingdom of Denmark emerged in the 8th century as a proficient seafaring nation in the struggle for control of the Baltic Sea. In 1397, Denmark joined Norway and Sweden to form the Kalmar Union, which persisted until Sweden's secession in 1523. The remaining union with Norway lasted until 1814. Denmark-Norway was one of the significant European naval powers during the 17th and 18th centuries. Following the Napoleonic Wars, Denmark ceded Norway to Sweden but kept the Faroe Islands, Iceland, and Greenland.\n\nThe 19th century saw a surge of nationalist movements throughout Europe, and a golden age of Danish culture. The first half of the 20th century was characterized by two world wars. In World War I, Denmark remained neutral and in World War II, it was occupied by Nazi Germany from 1940 to 1945. After the war, Denmark became a founding member of NATO and the United Nations.\n\nDenmark is a highly developed country, with the world's highest social mobility, a high level of income equality, the lowest perceived level of corruption in the world, the eleventh highest HDI in the world, one of the highest GDP per capita in the world, and one of the world's highest personal income tax rates. Denmark has the world's highest ratio of adults holding a tertiary education degree.\n\nDanish culture has been influential in various fields. The country is known for its design tradition, with Danish design being renowned for furniture, architecture, and household products. Danish design icons include Arne Jacobsen, Hans Wegner, and Verner Panton. The concept of hygge, roughly translated as cozy contentment, is central to Danish culture and has gained international recognition.\n\nDenmark has a highly developed mixed economy that is classified as a high-income economy by the World Bank. In 2017, it ranked as having the world's 18th highest per capita gross domestic product. The economy is characterized by extensive government welfare measures, and an equitable distribution of income. The largest industries in Denmark are pharmaceuticals, renewable energy, maritime shipping, and food processing. Denmark is a leading producer of wind energy and pork products.\n\nDanish cuisine traditionally consists of meat and fish dishes, often accompanied by rye bread and potatoes. Famous Danish foods include smørrebrød (open-faced sandwiches), frikadeller (meatballs), and Danish pastries. Copenhagen has become one of the gastronomic capitals of Europe, with restaurants like Noma being awarded multiple Michelin stars.",
  "fieldBoundaries": [
    {"docId": 6, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 6, "start": 8, "end": 3600, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Denmark"

# Estonia (7)
cat > "$DIR/7.json" << 'EOFCOUNTRY'
{
  "id": 7,
  "slug": "estonia",
  "title": "Estonia",
  "excerpt": "A Baltic nation known as one of the world's most digitally advanced societies",
  "href": "/countries/estonia",
  "type": "page",
  "category": "nordic",
  "text": "Estonia, officially the Republic of Estonia, is a country on the eastern coast of the Baltic Sea in Northern Europe. It is bordered to the north by the Gulf of Finland, across which lies Finland, to the west by the Baltic Sea, across which lies Sweden, to the south by Latvia, and to the east by Lake Peipus and Russia. The territory of Estonia consists of the mainland, the larger islands of Saaremaa and Hiiumaa, and over 2,200 other islands and islets on the eastern coast of the Baltic Sea, covering a total area of 45,339 square kilometers.\n\nTallinn, the capital and largest city, is situated on the northern coast of Estonia. The city is known for its medieval Old Town, which is a UNESCO World Heritage Site. Other major cities include Tartu, Narva, Pärnu, and Kohtla-Järve. The population of Estonia is approximately 1.4 million, making it one of the least populous member states of the European Union.\n\nAfter centuries of successive German, Danish, Swedish, and Russian rule, Estonia achieved independence in 1918. Initially a democratic state, Estonia was occupied by the Soviet Union in 1940 following the Molotov-Ribbentrop Pact of 1939. After World War II, Estonia was occupied again by the Soviet Union and its independence was not restored until 1991. Estonia is a developed country and a member of the European Union since 2004, the Eurozone since 2011, the OECD since 2010, and the Schengen Area since 2007. It is also a member of NATO since 2004.\n\nEstonia is one of the most digitally advanced societies in the world. It was the first country to offer e-Residency, a transnational digital identity available to anyone in the world. Estonia has implemented e-government services extensively, allowing citizens to vote, file taxes, start businesses, and access health records online. The X-Road data exchange layer enables secure data transfers across government services and has been adopted by other countries.\n\nThe Estonian economy is an advanced high-income economy and is a member of the European Union. Estonia's economy is characterized by a strong information technology sector, contributing significantly to the country's GDP. Tallinn is home to a number of international technology companies and startups, including Skype, which was founded in Estonia. The country has become known as a startup hub, with a strong ecosystem supporting entrepreneurship and innovation.\n\nEstonian culture is deeply influenced by its long history and Nordic heritage. The Estonian language is one of the few non-Indo-European languages spoken in Europe, belonging to the Finno-Ugric language family and is closely related to Finnish. Traditional Estonian culture features folk songs, dances, and crafts. The Estonian Song Festival, held every five years, is a UNESCO Masterpiece of the Oral and Intangible Heritage of Humanity and played a significant role in the Singing Revolution that led to Estonia regaining its independence.\n\nEstonia has extensive forests covering about 50 percent of its territory, making it one of the most forested countries in Europe. The country has a temperate climate with four distinct seasons. Tourism is a growing industry, with visitors attracted to Tallinn's medieval old town, the country's natural landscapes, and its spa resorts. Estonia is known for producing bog-grown berries, particularly cranberries and lingonberries, as well as dairy products and grain-based foods.",
  "fieldBoundaries": [
    {"docId": 7, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 7, "start": 8, "end": 3400, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Estonia"

# Finland (8)
cat > "$DIR/8.json" << 'EOFCOUNTRY'
{
  "id": 8,
  "slug": "finland",
  "title": "Finland",
  "excerpt": "A Nordic country known for its exceptional education system and thousands of lakes",
  "href": "/countries/finland",
  "type": "page",
  "category": "nordic",
  "text": "Finland, officially the Republic of Finland, is a Nordic country in Northern Europe. It shares land borders with Sweden to the northwest, Norway to the north, and Russia to the east, with the Gulf of Bothnia to the west and the Gulf of Finland across Estonia to the south. Finland covers an area of 338,455 square kilometers with a population of approximately 5.6 million. Helsinki is the capital and largest city, forming a larger metropolitan area with the neighboring cities of Espoo, Kauniainen, and Vantaa.\n\nThe vast majority of the population are ethnic Finns. Finnish, alongside Swedish, are the official languages. Sweden colonized the country during the Northern Crusades and Finnish, the language of most of the population, became a minority language. Finland was ceded to the Russian Empire in 1809, after which it was known as the Grand Duchy of Finland. In 1906, Finland became the first European state to grant all adults the right to vote and the first in the world to give all adult citizens the right to run for public office.\n\nFollowing the Russian Revolution in 1917, Finland declared itself independent. In 1918, the fledgling state was divided by civil war. During World War II, Finland fought the Soviet Union in the Winter War and the Continuation War, and Nazi Germany in the Lapland War. After the wars, Finland ceded parts of Karelia, Salla, Kuusamo, Petsamo, and islands in the Gulf of Finland to the Soviet Union.\n\nFinland joined the United Nations in 1955 and established an official policy of neutrality. The Finno-Soviet Treaty of 1948 gave the Soviet Union some leverage over Finnish domestic politics during the Cold War era. Finland joined the European Union in 1995 and the Eurozone at its inception in 1999. Finland joined NATO in 2023 in response to Russia's invasion of Ukraine, ending decades of military non-alignment.\n\nFinland is a top performer in numerous metrics of national performance, including education, economic competitiveness, civil liberties, quality of life, and human development. In 2015, Finland was ranked first in the World Human Capital and the Press Freedom Index. It consistently ranks among the happiest countries in the world according to the World Happiness Report.\n\nFinland is known as the land of a thousand lakes, though the actual number is nearly 188,000. The country has more lakes in relation to the size of its population than any other country. Finland is also heavily forested, with over 70 percent of its land covered by forests. The climate is cold with harsh winters, particularly in the north, and cool summers. The country experiences the midnight sun in summer and polar nights in winter in its northern regions.\n\nThe Finnish education system is consistently ranked among the best in the world. Education is free at all levels, from primary school to university, and school meals are provided. Teachers are highly trained and respected, requiring a master's degree to teach in primary schools. Finland's approach to education emphasizes equality, cooperation, and student well-being rather than competition and standardized testing.\n\nThe economy of Finland is highly industrialized and has a free-market economy. Key sectors include electronics, machinery, vehicles, and forest products. Finland is home to several multinational companies, including Nokia, Kone, and Stora Enso. The country is known for its strong social safety net, including universal healthcare and extensive social services. Finnish design, particularly in furniture, glassware, and textiles, has gained international recognition.",
  "fieldBoundaries": [
    {"docId": 8, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 8, "start": 8, "end": 3500, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Finland"

# France (9)
cat > "$DIR/9.json" << 'EOFCOUNTRY'
{
  "id": 9,
  "slug": "france",
  "title": "France",
  "excerpt": "A major Western European power known for its cultural influence, cuisine, and fashion",
  "href": "/countries/france",
  "type": "page",
  "category": "western",
  "text": "France, officially the French Republic, is a country located primarily in Western Europe. It also includes overseas regions and territories in the Americas and the Atlantic, Pacific and Indian Oceans, giving it one of the largest total exclusive economic zones in the world. Metropolitan France shares borders with Belgium and Luxembourg to the north, Germany to the northeast, Switzerland and Italy to the east, Spain and Andorra to the south. Its metropolitan area extends from the Rhine to the Atlantic Ocean and from the Mediterranean Sea to the English Channel and the North Sea.\n\nFrance has been a major power with strong cultural, economic, military, and political influence. A highly developed country, France is the world's seventh-largest economy by nominal GDP and ninth-largest by purchasing power parity. It is a permanent member of the United Nations Security Council, and a leading member state of the European Union and the Eurozone. France is also a member of the Group of 7, the North Atlantic Treaty Organization, the Organisation for Economic Co-operation and Development, and the World Trade Organization.\n\nFrance's population is approximately 68 million, with the capital Paris being the largest city and the country's cultural and economic center. Paris is renowned for its art museums and architectural landmarks such as the Eiffel Tower, the Louvre, Notre-Dame, the Arc de Triomphe, the Sacré-Cœur, and the Palace of Versailles. Other major cities include Marseille, Lyon, Toulouse, Nice, Nantes, Strasbourg, and Bordeaux.\n\nFrance has been a center of cultural creation for centuries and continues to be recognized worldwide for its rich cultural tradition. French art, cinema, fashion, and cuisine have influenced the world. The country has the most Nobel Prize winners in Literature, and has won the most number of Cannes Film Festival Palme d'Or awards. France is the world's top tourist destination, attracting over 90 million visitors annually.\n\nFrench cuisine is renowned globally for its sophistication and diversity. The country is famous for its wines, particularly from regions like Bordeaux, Burgundy, and Champagne. French culinary techniques form the basis of Western cooking, and the country has more Michelin-starred restaurants than any other nation. Traditional dishes include coq au vin, bouillabaisse, boeuf bourguignon, and countless varieties of cheese and bread.\n\nThe French economy is highly developed and diversified. Major industries include aerospace, automotive, luxury goods, cosmetics, food and beverages, and pharmaceuticals. France is a leader in nuclear energy, with nuclear power accounting for about 70 percent of its electricity production. The country has a strong tourism sector, which is one of the largest in the world by international visitor arrivals.\n\nFrance has a rich history dating back to the Gauls, a Celtic people. After Roman conquest, Gaul became a major part of the Roman Empire. The medieval period saw the rise of the French monarchy and its conflict with England in the Hundred Years' War. The French Revolution of 1789 was a turning point in world history, leading to the rise of Napoleon Bonaparte and later to the establishment of the French Republic.\n\nFrench culture emphasizes philosophy, literature, art, and science. France has produced renowned thinkers like René Descartes, Voltaire, Jean-Jacques Rousseau, and Jean-Paul Sartre. French literature has given the world Victor Hugo, Alexandre Dumas, Marcel Proust, and Albert Camus. The Impressionist movement in art originated in France with painters like Claude Monet, Pierre-Auguste Renoir, and Edgar Degas.",
  "fieldBoundaries": [
    {"docId": 9, "start": 0, "end": 6, "fieldType": "title", "sectionId": null},
    {"docId": 9, "start": 7, "end": 3700, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated France"

# Germany (10)
cat > "$DIR/10.json" << 'EOFCOUNTRY'
{
  "id": 10,
  "slug": "germany",
  "title": "Germany",
  "excerpt": "Europe's largest economy, known for its engineering excellence and rich cultural history",
  "href": "/countries/germany",
  "type": "page",
  "category": "central",
  "text": "Germany, officially the Federal Republic of Germany, is a country in Central Europe. It is the second most populous country in Europe after Russia, and the most populous member state of the European Union. Germany is situated between the Baltic and North seas to the north and the Alps to the south. It borders Denmark to the north, Poland and the Czech Republic to the east, Austria and Switzerland to the south, and France, Luxembourg, Belgium and the Netherlands to the west. The nation's capital and most populous city is Berlin.\n\nVarious Germanic tribes have inhabited the northern parts of modern Germany since classical antiquity. A region named Germania was documented before AD 100. In the 10th century, German territories formed a central part of the Holy Roman Empire. During the 16th century, northern German regions became the center of the Protestant Reformation. Following the Napoleonic Wars and the dissolution of the Holy Roman Empire, the German Confederation was formed in 1815.\n\nIn 1871, Germany became a nation-state when most of the German states unified into the Prussian-dominated German Empire. After World War I and the German Revolution of 1918–1919, the Empire was replaced by the semi-presidential Weimar Republic. The Nazi seizure of power in 1933 led to the establishment of a totalitarian dictatorship, World War II, and the Holocaust. After the end of World War II in Europe and a period of Allied occupation, Germany was divided into the Federal Republic of Germany (West Germany) and the German Democratic Republic (East Germany). The country was reunified on 3 October 1990.\n\nToday, Germany has a social market economy with a highly skilled labour force, a low level of corruption, and a high level of innovation. It is the world's third largest exporter and importer of goods. The country has developed a comprehensive system of social security. It holds a key position in European affairs and maintains a multitude of close partnerships on a global level. Germany is recognized as a scientific and technological leader in several fields.\n\nGerman engineering is world-renowned, particularly in the automotive industry. Major German car manufacturers include Volkswagen, BMW, Mercedes-Benz, Audi, and Porsche. Germany is also a leader in mechanical engineering, chemical production, and renewable energy technology. The country has made the transition to renewable energy a priority, with significant investments in wind and solar power.\n\nGerman culture has spanned the entire German-speaking world. From its roots, culture in Germany has been shaped by major intellectual and popular currents in Europe, both religious and secular. Germany is known as the land of poets and thinkers, having produced influential figures such as Johann Wolfgang von Goethe, Friedrich Schiller, and Immanuel Kant.\n\nGerman cuisine varies from region to region but is generally characterized by hearty dishes. Sausages (Würste), beer, bread, and pork are staples of German cuisine. Bavaria is particularly known for its beer culture and traditional dishes like weisswurst and pretzels. Germany has one of the world's oldest and most renowned beer traditions, with over 1,300 breweries producing more than 5,000 brands of beer.\n\nGermany has a universal multi-payer healthcare system and compulsory education. The country's education system is highly decentralized, with each of the 16 states responsible for its own educational system. German universities include some of the oldest in the world, such as Heidelberg University (founded 1386) and the University of Leipzig (founded 1409). Germany continues to be a leader in scientific research and innovation.",
  "fieldBoundaries": [
    {"docId": 10, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 10, "start": 8, "end": 3700, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Germany"

# Greece (11)
cat > "$DIR/11.json" << 'EOFCOUNTRY'
{
  "id": 11,
  "slug": "greece",
  "title": "Greece",
  "excerpt": "The birthplace of Western civilization, democracy, and philosophy",
  "href": "/countries/greece",
  "type": "page",
  "category": "mediterranean",
  "text": "Greece, officially the Hellenic Republic, is a country in Southeast Europe. It is situated on the southern tip of the Balkans, and is located at the crossroads of Europe, Asia, and Africa. Greece shares land borders with Albania to the northwest, North Macedonia and Bulgaria to the north, and Turkey to the northeast. The Aegean Sea lies to the east of the mainland, the Ionian Sea to the west, and the Sea of Crete and the Mediterranean Sea to the south. Greece has the longest coastline on the Mediterranean Basin, featuring thousands of islands.\n\nGreece is considered the cradle of Western civilization, being the birthplace of democracy, Western philosophy, Western literature, historiography, political science, major scientific and mathematical principles, theatre and the Olympic Games. Greece's rich historical legacy is reflected by its 18 UNESCO World Heritage Sites. Greece is a democratic and developed country with an advanced high-income economy and a very high standard of living.\n\nGreece is a unitary parliamentary republic and a founding member of the United Nations, a member of the European Union since 1981, and part of the Eurozone since 2001. It is also a member of numerous other international institutions, including the Council of Europe, the North Atlantic Treaty Organization, the Organisation for Economic Co-operation and Development, and the Organization of the Black Sea Economic Cooperation.\n\nThe population of Greece is approximately 10.4 million as of 2021, with Athens being the nation's capital and largest city. Other major cities include Thessaloniki, Patras, Heraklion, and Larissa. The Greek language has been spoken in Greece for more than 3,000 years, making it one of the oldest recorded living languages in the world. Modern Greek has evolved from ancient Greek through various stages.\n\nAncient Greek civilization produced some of history's greatest philosophers, including Socrates, Plato, and Aristotle. The city-states of Athens and Sparta were prominent during the Classical period. The Parthenon on the Acropolis in Athens remains one of the most iconic symbols of ancient Greek achievement. Ancient Greece also gave the world the first Olympics, held in Olympia in 776 BC.\n\nThe Byzantine Empire, centered in Constantinople, was the continuation of the Roman Empire in its eastern provinces and preserved Greek culture for over a millennium. After the fall of Constantinople in 1453, Greece came under Ottoman rule for nearly 400 years. The Greek War of Independence in 1821-1829 led to the establishment of the modern Greek state.\n\nGreek cuisine is part of the Mediterranean diet and is known for its use of olive oil, vegetables, herbs, grains, bread, wine, fish, and various meats. Traditional dishes include moussaka, souvlaki, dolmades, and Greek salad. Greece is one of the largest producers of olive oil in the world and Greek olive oil is highly prized for its quality.\n\nTourism is a major sector of the Greek economy. Greece's numerous islands, including Crete, Rhodes, Corfu, Santorini, and Mykonos, are popular tourist destinations. The country offers archaeological sites, beaches, and a Mediterranean climate that attracts millions of visitors each year. Greek shipping is also a major industry, with Greece having one of the largest merchant fleets in the world.",
  "fieldBoundaries": [
    {"docId": 11, "start": 0, "end": 6, "fieldType": "title", "sectionId": null},
    {"docId": 11, "start": 7, "end": 3500, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Greece"

# Hungary (12)
cat > "$DIR/12.json" << 'EOFCOUNTRY'
{
  "id": 12,
  "slug": "hungary",
  "title": "Hungary",
  "excerpt": "A Central European country known for its thermal baths and historic Budapest",
  "href": "/countries/hungary",
  "type": "page",
  "category": "central",
  "text": "Hungary is a landlocked country in Central Europe. Spanning 93,030 square kilometers in the Carpathian Basin, it is bordered by Slovakia to the north, Ukraine to the northeast, Romania to the east and southeast, Serbia to the south, Croatia and Slovenia to the southwest, and Austria to the west. Hungary has a population of nearly 10 million, mostly ethnic Hungarians and a significant Romani minority. Hungarian, the official language, is the most widely spoken Uralic language in the world.\n\nBudapest is the capital and largest city, and is considered one of the most beautiful cities in Europe. The city is famous for its historic architecture, including the Hungarian Parliament Building, Buda Castle, Fisherman's Bastion, and the Chain Bridge spanning the Danube River. Budapest's thermal baths, fed by over 120 hot springs, have been popular since Roman times.\n\nHungary's history begins with the arrival of the Magyars in the Carpathian Basin in the 9th century. The Kingdom of Hungary was founded in 1000 AD by King Stephen I, who also converted the nation to Christianity. For several centuries, Hungary was a major European power. After the Battle of Mohács in 1526, the country was divided among the Ottoman Empire, the Habsburg Monarchy, and the Principality of Transylvania.\n\nFollowing World War I and the dissolution of Austria-Hungary, Hungary lost approximately two-thirds of its territory and one-third of its population under the Treaty of Trianon. After World War II, Hungary became a satellite state of the Soviet Union, leading to four decades of communist rule. The 1956 Hungarian Revolution was a nationwide revolt against the communist government that was suppressed by Soviet forces.\n\nHungary's transition to a market economy following the fall of the Iron Curtain in 1989 led to its accession to the European Union in 2004. Today, Hungary is a middle power and has the world's 57th largest economy by nominal GDP. The country attracts millions of tourists annually, who come to visit Budapest's historic sites, the wine regions of Tokaj and Eger, Lake Balaton (the largest lake in Central Europe), and the Great Hungarian Plain.\n\nHungarian culture is rich and distinctive. Hungarian music ranges from traditional folk melodies to the works of famous composers like Franz Liszt, Béla Bartók, and Zoltán Kodály. Hungarian literature has produced notable authors such as Imre Kertész, winner of the Nobel Prize in Literature. Hungarian cuisine is known for its use of paprika, with dishes like goulash, chicken paprikash, and lángos being popular.\n\nThe Hungarian language is unique in Europe, being part of the Uralic language family rather than Indo-European. It has no close relatives among European languages, though Finnish and Estonian are its most closely related languages. Hungarian is known for its complex grammar and large vocabulary.\n\nHungary has made significant contributions to science and mathematics. Hungarian scientists include Nobel Prize winners like Albert Szent-Györgyi, Georg von Békésy, and John von Neumann (who made fundamental contributions to computing and mathematics). The country has a strong tradition in engineering and technology, with Hungarians contributing to the development of the ballpoint pen, holography, and computer science.",
  "fieldBoundaries": [
    {"docId": 12, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 12, "start": 8, "end": 3400, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Hungary"

# Iceland (13)
cat > "$DIR/13.json" << 'EOFCOUNTRY'
{
  "id": 13,
  "slug": "iceland",
  "title": "Iceland",
  "excerpt": "A volcanic island nation known for dramatic landscapes and geothermal energy",
  "href": "/countries/iceland",
  "type": "page",
  "category": "nordic",
  "text": "Iceland is a Nordic island country in the North Atlantic Ocean, with a population of about 380,000 and an area of 103,000 square kilometers, making it the most sparsely populated country in Europe. The capital and largest city is Reykjavík, with the surrounding areas in the southwest of the country being home to over two-thirds of the population. Iceland is the only part of the Mid-Atlantic Ridge that rises above sea level, and its central volcanic plateau is nearly constantly erupting.\n\nThe interior consists of a plateau characterized by sand and lava fields, mountains, and glaciers, and many glacial rivers flow to the sea through the lowlands. Iceland is warmed by the Gulf Stream and has a temperate climate, despite a high latitude just outside the Arctic Circle. Its high latitude and marine influence keep summers chilly, with most of the inhabited lowland areas having a tundra climate.\n\nAccording to the ancient manuscript Landnámabók, the settlement of Iceland began in 874 AD when the Norwegian chieftain Ingólfr Arnarson became the first permanent settler on the island. Others had visited the island earlier and stayed over winter. Over the following centuries, Norsemen settled Iceland, bringing with them thralls of Gaelic origin. From 1262 to 1918, Iceland was part of the Norwegian and later the Danish monarchies. The country's independence movement, which took a peaceful course, led to independence from Denmark in 1918 and the establishment of a republic in 1944.\n\nIceland is a highly developed country and consistently ranks high in measures of stability, equality, and livability. It is a member of the United Nations, NATO, EFTA, the Arctic Council, the Council of Europe, and the OECD, but not of the European Union. The Icelandic economy is small but technologically advanced, with a GDP per capita among the highest in the world. The country runs almost entirely on renewable energy, with geothermal and hydroelectric power providing over 80 percent of primary energy.\n\nIceland is famous for its dramatic landscapes, which include volcanoes, geysers, hot springs, lava fields, glaciers, and waterfalls. The country has over 130 volcanoes, of which about 30 are active. Notable eruptions include the 2010 Eyjafjallajökull eruption, which disrupted air travel across Europe. Geothermal activity is harnessed for heating buildings and generating electricity. The Blue Lagoon, a geothermal spa, is one of Iceland's most visited attractions.\n\nIcelandic culture is rooted in North Germanic traditions. The Icelandic language is an Indo-European language belonging to the North Germanic branch. It is most closely related to Faroese and Norwegian, and has changed relatively little since medieval times, meaning modern Icelanders can still read the Old Norse sagas. These medieval texts, including the Poetic Edda and the Prose Edda, are important sources of Norse mythology and Viking history.\n\nIceland has a vibrant arts scene and has produced internationally recognized musicians like Björk and Sigur Rós. The country celebrates its literary heritage, with a tradition of giving books as Christmas gifts known as Jólabókaflóð (Christmas Book Flood). Iceland has one of the highest literacy rates in the world and publishes more books per capita than any other country. Traditional Icelandic cuisine includes fermented shark, dried fish, and lamb, while modern Icelandic restaurants have gained recognition for innovative Nordic cuisine.",
  "fieldBoundaries": [
    {"docId": 13, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 13, "start": 8, "end": 3500, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Iceland"

# Ireland (14)
cat > "$DIR/14.json" << 'EOFCOUNTRY'
{
  "id": 14,
  "slug": "ireland",
  "title": "Ireland",
  "excerpt": "An island nation known for its literary tradition, music, and green landscapes",
  "href": "/countries/ireland",
  "type": "page",
  "category": "western",
  "text": "Ireland, also known as the Republic of Ireland, is a country in northwestern Europe consisting of 26 of the 32 counties of the island of Ireland. The capital and largest city is Dublin, which is located on the eastern side of the island. Around 40 percent of the country's population of 5 million people resides in the Greater Dublin Area. The sovereign state shares its only land border with Northern Ireland, which is part of the United Kingdom.\n\nThe Irish Sea lies between Great Britain and Ireland. Ireland is surrounded by the Atlantic Ocean, with the Celtic Sea to the south, St George's Channel to the southeast, and the Irish Sea to the east. It is separated from Great Britain by the Irish Sea. The island has a mild but changeable oceanic climate with few extremes. The island is characterized by a lush, green landscape, earning it the nickname the Emerald Isle.\n\nIreland's history is marked by periods of Celtic, Viking, Norman, and English influence. Celtic Ireland was organized into dozens of small kingdoms. The Vikings established settlements including Dublin in the 9th century. The Norman invasion in 1169 led to centuries of English and later British rule. The Great Famine of 1845-1852, caused by potato blight, led to mass starvation and emigration, with the population declining from about 8 million to about 6 million.\n\nThe Easter Rising of 1916 and the subsequent Irish War of Independence led to the establishment of the Irish Free State in 1922. The country became a republic in 1949 and joined the European Economic Community (now the European Union) in 1973. The Celtic Tiger period of rapid economic growth from the mid-1990s to 2007 transformed Ireland from one of Europe's poorest countries to one of its wealthiest.\n\nIreland has a rich literary tradition and has produced four Nobel Prize winners in Literature: W.B. Yeats, George Bernard Shaw, Samuel Beckett, and Seamus Heaney. Other famous Irish writers include James Joyce, Oscar Wilde, Jonathan Swift, and Bram Stoker. Dublin is a UNESCO City of Literature. Irish music, including traditional folk music and modern rock, has gained international recognition through artists like U2, The Cranberries, and Enya.\n\nThe Irish language (Gaeilge) is the first official language of the state, though English is more commonly spoken. Areas where Irish is spoken as a community language are known as Gaeltacht regions, mainly located along the western seaboard. The government has policies to promote Irish language use and bilingual education.\n\nIreland's economy is highly globalized and relies heavily on foreign direct investment. It is a significant hub for pharmaceutical and technology companies due to favorable corporate tax rates. Major employers include Apple, Google, Facebook (Meta), Intel, and Pfizer. Traditional industries include agriculture, food processing, and tourism. Irish whiskey, Guinness stout, and Irish cream liqueurs are exported worldwide.\n\nIrish culture is known for its pubs, which serve as community gathering places. Traditional Irish cuisine includes dishes like Irish stew, soda bread, and colcannon. The country is famous for its traditional music sessions (seisiúns), step dancing, and the St. Patrick's Day celebrations that have spread to Irish communities worldwide. GAA sports, particularly hurling and Gaelic football, are uniquely Irish sports that remain popular throughout the country.",
  "fieldBoundaries": [
    {"docId": 14, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 14, "start": 8, "end": 3500, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Ireland"

# Italy (15)
cat > "$DIR/15.json" << 'EOFCOUNTRY'
{
  "id": 15,
  "slug": "italy",
  "title": "Italy",
  "excerpt": "A Mediterranean country with the world's largest heritage of art and monuments",
  "href": "/countries/italy",
  "type": "page",
  "category": "mediterranean",
  "text": "Italy, officially the Italian Republic, is a country in Southern and Western Europe. Located in the middle of the Mediterranean Sea, it consists of a peninsula delimited by the Alps and several islands surrounding it. Italy shares land borders with France, Switzerland, Austria, Slovenia, and the enclaved microstates of Vatican City and San Marino. It has a territorial exclave in Switzerland, Campione, and a maritime exclave in Tunisian waters, Lampedusa.\n\nItaly covers an area of 301,340 square kilometers and has a largely temperate seasonal and Mediterranean climate. With around 60 million inhabitants, Italy is the third-most populous member state of the European Union. The capital and largest city is Rome; other major cities include Milan, Naples, Turin, Palermo, Genoa, Bologna, Florence, and Venice.\n\nItaly has been the home of many European cultures and peoples, such as the Italic peoples, the Etruscans, and the Greeks. The Roman Empire, which was based in Rome, left a lasting legacy in Western civilization. During the Early Middle Ages, Italy endured social and political turmoil caused by the fall of the Western Roman Empire and barbarian invasions. By the 11th century, Italian city-states, most notably Florence, Genoa, Milan, and Venice, rose to become the leading economic powers in Europe.\n\nThe Italian Renaissance period, which flourished from the 14th through the 16th centuries, saw the emergence of revolutionary ideas in science, art, and architecture. Renaissance figures such as Leonardo da Vinci, Michelangelo, Raphael, Galileo Galilei, and Machiavelli made immense contributions to Western culture. Italy has the world's largest number of UNESCO World Heritage Sites and is the fifth most visited country in the world.\n\nAfter centuries of political fragmentation and foreign domination, Italy was unified in 1861, becoming a constitutional monarchy. Italy entered World War I on the side of the Allies and later became a fascist state under Benito Mussolini in the 1920s. After World War II, the Italian monarchy was abolished, and a democratic republic was established. Italy was a founding member of the European Economic Community (EEC), now the European Union.\n\nItalian culture has had a profound influence on Western civilization. Italy is known worldwide for its art, architecture, fashion, opera, literature, cinema, and cuisine. Italian art treasures include works by Leonardo da Vinci, Michelangelo, Botticelli, and Caravaggio. The country is home to famous landmarks such as the Colosseum, the Leaning Tower of Pisa, the Vatican, and the canals of Venice.\n\nItalian cuisine is one of the most popular in the world. Regional variations are significant, with Northern Italian cuisine differing from Southern Italian cuisine. Famous Italian dishes include pizza, pasta, risotto, gelato, and espresso coffee. Italy is also renowned for its wine production, with regions like Tuscany, Piedmont, and Veneto producing world-class wines.\n\nThe Italian economy is the third-largest in the Eurozone and the eighth-largest in the world. Key industries include tourism, fashion, automotive (Ferrari, Lamborghini, Fiat), food and beverages, machinery, and luxury goods. Italy is a leader in fashion design, with Milan being one of the world's fashion capitals alongside Paris, London, and New York. Italian fashion houses like Gucci, Prada, Armani, and Versace are globally recognized.",
  "fieldBoundaries": [
    {"docId": 15, "start": 0, "end": 5, "fieldType": "title", "sectionId": null},
    {"docId": 15, "start": 6, "end": 3600, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Italy"

# Latvia (16)
cat > "$DIR/16.json" << 'EOFCOUNTRY'
{
  "id": 16,
  "slug": "latvia",
  "title": "Latvia",
  "excerpt": "A Baltic state known for its Art Nouveau architecture and pristine nature",
  "href": "/countries/latvia",
  "type": "page",
  "category": "nordic",
  "text": "Latvia, officially the Republic of Latvia, is a country in the Baltic region of Northern Europe. It is bordered by Estonia to the north, Lithuania to the south, Russia to the east, Belarus to the southeast, and has a maritime border with Sweden to the west. Latvia has a population of 1.9 million and a territory spanning 64,589 square kilometers. The country has a temperate seasonal climate. Its capital and largest city is Riga, home to about one-third of the country's population.\n\nLatvia has been inhabited since approximately 9000 BC. The region was home to various Baltic tribes, including the Curonians, Latgalians, Selonians, and Semigallians. German crusaders conquered the area in the early 13th century, establishing the Livonian Order. The region was later controlled by Poland, Sweden, and Russia. Latvia declared independence from Russia on November 18, 1918, but was occupied by the Soviet Union in 1940 and later by Nazi Germany in 1941-1944. Soviet occupation resumed after World War II and lasted until 1991.\n\nLatvia restored its independence on August 21, 1991, following the Singing Revolution, a peaceful movement in the Baltic states. The country transitioned from a Soviet-style command economy to a market economy, experiencing significant economic growth in the early 2000s. Latvia joined NATO and the European Union in 2004 and adopted the euro in 2014.\n\nRiga, the capital, is the largest city in the Baltic states and is known for its historic center, which is a UNESCO World Heritage Site. The city features one of the finest collections of Art Nouveau architecture in the world, with over 750 buildings in this style. Other notable features include the medieval old town, the Freedom Monument, and the Central Market.\n\nLatvian culture has been influenced by its geographic position between Scandinavia, Russia, and Germany. Traditional Latvian culture includes distinctive folk songs (dainas), dances, and crafts. The Latvian Song and Dance Festival, held every five years since 1873, is a major cultural event that brings together thousands of participants and has been recognized by UNESCO as a Masterpiece of the Oral and Intangible Heritage of Humanity.\n\nThe Latvian language belongs to the Baltic branch of the Indo-European language family. It is one of only two surviving Baltic languages (along with Lithuanian) and has retained many archaic features of Proto-Indo-European. The language uses the Latin script with additional letters and diacritical marks.\n\nLatvia's economy is a service-based economy with significant contributions from information technology, timber and wood processing, food processing, and manufacturing. The country has invested heavily in technology and is known for having some of the fastest internet speeds in the world. Tourism, particularly to Riga's historic center and the country's natural landscapes, is an important sector.\n\nLatvia is covered with extensive forests (about 54 percent of its territory), making it one of the greenest countries in Europe. The country has over 12,000 rivers and 2,000 lakes. The Baltic Sea coastline stretches for 531 kilometers and features wide, sandy beaches. Latvia's diverse natural habitats support a wide range of wildlife, including the European bison, which has been reintroduced to Latvian forests.",
  "fieldBoundaries": [
    {"docId": 16, "start": 0, "end": 6, "fieldType": "title", "sectionId": null},
    {"docId": 16, "start": 7, "end": 3300, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Latvia"

# Lithuania (17)
cat > "$DIR/17.json" << 'EOFCOUNTRY'
{
  "id": 17,
  "slug": "lithuania",
  "title": "Lithuania",
  "excerpt": "The largest Baltic state with a rich medieval history and baroque Vilnius",
  "href": "/countries/lithuania",
  "type": "page",
  "category": "nordic",
  "text": "Lithuania, officially the Republic of Lithuania, is a country in the Baltic region of Europe. It is one of three Baltic states and lies on the eastern shore of the Baltic Sea. Lithuania shares land borders with Latvia to the north, Belarus to the east and south, Poland to the south, and Kaliningrad Oblast of Russia to the southwest. Lithuania has an estimated population of 2.8 million as of 2021, and its capital and largest city is Vilnius.\n\nThe name Lithuania was first mentioned in 1009 in a medieval manuscript. Lithuanians are an ethnic group of the Baltic nations. Lithuanian, one of two surviving Baltic languages, is the official language. Lithuania is historically Lutheran and Roman Catholic. During the 14th and 15th centuries, Lithuania was the largest country in Europe, stretching from the Baltic Sea to the Black Sea. The Grand Duchy of Lithuania was a significant power in medieval Europe.\n\nThe personal union with Poland that began in 1386 led to the establishment of the Polish-Lithuanian Commonwealth in 1569, one of the largest and most populous countries of 16th to 17th century Europe. The Commonwealth was partitioned in 1795, with Lithuania becoming part of the Russian Empire. Following World War I, Lithuania declared independence on February 16, 1918. The interwar Lithuanian state was ended by Soviet occupation in 1940.\n\nGermany occupied Lithuania during World War II. After the war, Soviet rule was re-established, and Lithuania was incorporated into the USSR. On March 11, 1990, Lithuania became the first Soviet republic to declare independence, though this was not recognized internationally until September 1991. Lithuania joined NATO and the European Union in 2004 and the Eurozone in 2015.\n\nVilnius, the capital, is a UNESCO World Heritage Site known for its baroque architecture. The historic center features over 1,500 buildings spanning Gothic, Renaissance, Baroque, and Neoclassical styles. Vilnius University, founded in 1579, is one of the oldest and most distinguished universities in Northern Europe.\n\nLithuanian culture is characterized by its folk traditions, which include distinctive songs (dainos), dances, and crafts. The Lithuanian language is considered one of the most archaic living Indo-European languages, preserving many features of Proto-Indo-European. Lithuania has a strong basketball tradition and is often referred to as a basketball country. The national team has won multiple European championships and Olympic medals.\n\nThe Lithuanian economy has undergone significant transformation since independence. It has transitioned from a Soviet-planned economy to a market economy. Key sectors include manufacturing, services, and information technology. Lithuania has a highly educated workforce and has developed a strong technology sector. The country is known for producing laser technology and has a growing biotechnology industry.\n\nLithuania's landscape is characterized by lowlands with many lakes, rivers, and forests. About one-third of the country is covered with forests. The Curonian Spit, a UNESCO World Heritage Site shared with Russia, features unique sand dune landscapes. Traditional Lithuanian cuisine includes cepelinai (potato dumplings), šaltibarščiai (cold beet soup), and various potato and meat dishes. Lithuania is also known for its amber, which is found along the Baltic coast.",
  "fieldBoundaries": [
    {"docId": 17, "start": 0, "end": 9, "fieldType": "title", "sectionId": null},
    {"docId": 17, "start": 10, "end": 3300, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Lithuania"

# Luxembourg (18)
cat > "$DIR/18.json" << 'EOFCOUNTRY'
{
  "id": 18,
  "slug": "luxembourg",
  "title": "Luxembourg",
  "excerpt": "A small, wealthy nation and major European financial center",
  "href": "/countries/luxembourg",
  "type": "page",
  "category": "western",
  "text": "Luxembourg, officially the Grand Duchy of Luxembourg, is a small landlocked country in Western Europe. It is bordered by Belgium to the west and north, Germany to the east, and France to the south. Its capital, Luxembourg City, is one of the four official capitals of the European Union and the seat of the European Court of Justice. Luxembourg has a population of approximately 650,000 within an area of 2,586 square kilometers, making it one of the smallest sovereign states in Europe.\n\nLuxembourg's culture, people, and languages are highly intertwined with its neighbors, making it a mixture of French and Germanic cultures. French is the most commonly written and spoken language, while German is also widely used. Luxembourgish, a Moselle-Franconian dialect of German, is the national language. Most Luxembourgers speak at least three languages fluently.\n\nThe history of Luxembourg is considered to begin in 963 when Count Siegfried I acquired a rocky promontory and its Roman-era fortifications known as Lucilinburhuc, around which a town gradually developed. The House of Luxembourg became one of the most powerful dynasties in medieval Europe, producing several Holy Roman Emperors. The country lost much of its territory in the 19th century but gained independence in 1839.\n\nLuxembourg is a founding member of the European Union, NATO, OECD, the United Nations, Benelux, and the Western European Union. The city of Luxembourg is one of the de facto capitals of the European Union, hosting the European Court of Justice, the European Investment Bank, and several other important EU institutions. Its historical importance is reflected in its UNESCO World Heritage status.\n\nThe economy of Luxembourg is largely dependent on the banking, steel, and industrial sectors. Luxembourg has the highest GDP per capita in the world according to the World Bank and IMF. The country is a leading financial center, with over 150 banks and numerous fund management companies. It is known for favorable tax policies that have attracted many multinational corporations.\n\nLuxembourg City features a dramatic landscape with its old town situated on a rocky outcrop overlooking deep gorges cut by the Alzette and Pétrusse rivers. The city's fortifications, built over a 400-year period, earned it the nickname Gibraltar of the North. Today, many of the fortifications have been converted into parks and walking paths.\n\nLuxembourgish cuisine reflects the country's position between France, Belgium, and Germany. Traditional dishes include judd mat gaardebounen (smoked pork collar with broad beans), bouneschlupp (green bean soup), and gromperekichelcher (potato pancakes). The country also has a wine tradition along the Moselle River, producing primarily white wines.\n\nLuxembourg has invested heavily in technology and has become a hub for satellite communications. SES, one of the world's largest satellite operators, is headquartered in Luxembourg. The country is also developing its space industry and was the first European country to establish a legal framework for space resource utilization. Despite its small size, Luxembourg has punched above its weight in European and global affairs.",
  "fieldBoundaries": [
    {"docId": 18, "start": 0, "end": 10, "fieldType": "title", "sectionId": null},
    {"docId": 18, "start": 11, "end": 3200, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Luxembourg"

# Malta (19)
cat > "$DIR/19.json" << 'EOFCOUNTRY'
{
  "id": 19,
  "slug": "malta",
  "title": "Malta",
  "excerpt": "A small Mediterranean archipelago with ancient temples and strategic history",
  "href": "/countries/malta",
  "type": "page",
  "category": "mediterranean",
  "text": "Malta, officially the Republic of Malta, is an island country consisting of an archipelago in the Mediterranean Sea. It lies 80 kilometers south of Italy, 284 kilometers east of Tunisia, and 333 kilometers north of Libya. The country covers just over 316 square kilometers, making it one of the world's smallest and most densely populated countries. The capital is Valletta, which is the smallest national capital in the European Union by area. The main island, also called Malta, is the largest of the three major islands that constitute the country.\n\nMalta has been inhabited since approximately 5900 BC. A succession of powers, including the Phoenicians, Romans, Byzantines, Arabs, and Normans, have ruled the islands. The Knights of St. John held Malta from 1530 to 1798. The island was subsequently controlled by France and later Britain. Malta gained independence from Britain in 1964 and became a republic in 1974. It joined the European Union in 2004 and the Eurozone in 2008.\n\nMalta is home to some of the oldest free-standing structures in the world. The megalithic temples of Ġgantija on Gozo and Ħaġar Qim, Mnajdra, and Tarxien on Malta are UNESCO World Heritage Sites and date back to around 3600 BC, making them older than Stonehenge and the Egyptian pyramids. The Hypogeum of Ħal-Saflieni, an underground temple complex dating to about 4000 BC, is another UNESCO site.\n\nThe Knights of St. John, also known as the Knights Hospitaller, made Malta their home after being expelled from Rhodes by the Ottoman Empire. They fortified the islands and built Valletta, which became one of the most heavily fortified cities in the world. The Great Siege of Malta in 1565, when the Knights repelled an Ottoman invasion, is a defining moment in Maltese history.\n\nMalta has two official languages: Maltese and English. Maltese is the only Semitic language written in the Latin script and is descended from Siculo-Arabic, with significant influences from Italian, particularly Sicilian. The majority of Maltese people are Roman Catholic, and the Church plays a significant role in society.\n\nThe Maltese economy is primarily based on services, particularly financial services, tourism, and remote gaming. Malta has become a major hub for online gambling companies due to its regulatory framework. The film industry has grown significantly, with major productions filmed on the islands. Tourism is a major economic driver, with millions of visitors annually attracted by the Mediterranean climate, historic sites, and beaches.\n\nMaltese cuisine reflects the island's history, with influences from Sicilian, British, and North African cuisines. Traditional dishes include pastizzi (savory pastries), rabbit stew (stuffat tal-fenek), and fish soup (aljotta). The islands are known for their local wines and traditional honey rings.\n\nValletta, the capital, is a UNESCO World Heritage Site and was European Capital of Culture in 2018. The city features baroque architecture, numerous churches, and historic fortifications. St. John's Co-Cathedral houses Caravaggio's masterpiece, The Beheading of Saint John the Baptist. The Three Cities (Vittoriosa, Senglea, and Cospicua) across the Grand Harbour preserve much of Malta's maritime heritage.",
  "fieldBoundaries": [
    {"docId": 19, "start": 0, "end": 5, "fieldType": "title", "sectionId": null},
    {"docId": 19, "start": 6, "end": 3300, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Malta"

# Netherlands (20)
cat > "$DIR/20.json" << 'EOFCOUNTRY'
{
  "id": 20,
  "slug": "netherlands",
  "title": "Netherlands",
  "excerpt": "A low-lying nation known for windmills, tulips, cycling, and liberal policies",
  "href": "/countries/netherlands",
  "type": "page",
  "category": "western",
  "text": "The Netherlands, informally Holland, is a country primarily located in Northwestern Europe. It is the largest of four constituent countries of the Kingdom of the Netherlands. The Netherlands consists of twelve provinces, and borders Germany to the east, Belgium to the south, and the North Sea to the northwest. It shares maritime borders with both countries and the United Kingdom. The country is known for its flat landscape of canals, tulip fields, windmills, and cycling routes.\n\nThe Netherlands has a population of approximately 17.5 million and is one of the most densely populated countries in the world. Amsterdam is the country's capital, while The Hague holds the seat of the States General, Cabinet and Supreme Court. Rotterdam hosts Europe's largest port. The country's name literally means the lower countries, referring to its flat and low elevation, with about 26 percent of its area below sea level.\n\nThe Dutch have been at the forefront of water management technology for centuries. About one-third of the Netherlands is protected by dikes and sea defenses. The Delta Works, a series of construction projects in the southwest of the Netherlands, is one of the largest hydraulic engineering projects ever undertaken. These systems protect the country from the North Sea and from river flooding.\n\nDuring the Dutch Golden Age in the 17th century, the Netherlands was the world's leading maritime and commercial power. The Dutch East India Company was the first multinational corporation and pioneered many business practices. Dutch painters such as Rembrandt van Rijn, Johannes Vermeer, and Vincent van Gogh are among the most famous artists in Western history. The country also made significant contributions to science, with figures like Christiaan Huygens and Antonie van Leeuwenhoek.\n\nThe Netherlands is a constitutional monarchy with a parliamentary system. It is a founding member of the European Union, Eurozone, G10, NATO, OECD, WTO, and the Benelux Union. The country is known for its progressive social policies, including the legalization of euthanasia and same-sex marriage. Amsterdam is famous for its tolerance and has historically been a haven for refugees and dissidents.\n\nThe Dutch economy is the 17th largest in the world. It is a major player in international trade and is home to many multinational corporations, including Royal Dutch Shell, Philips, Unilever, and ING. The Port of Rotterdam is the largest seaport in Europe. Agriculture, particularly dairy and horticulture, remains important; the Netherlands is the world's second-largest agricultural exporter.\n\nDutch culture is reflected in its art, music, and traditions. The country is famous for its traditional Delft Blue pottery, wooden clogs (klompen), and cheese markets. Traditional Dutch cuisine includes stroopwafels, poffertjes, bitterballen, and herring. The Netherlands is also known for its cycling culture, with more bicycles than people and extensive cycling infrastructure.\n\nThe Netherlands has a highly educated population and ranks high in quality of education. Dutch universities, including the University of Amsterdam, Leiden University, and Delft University of Technology, are internationally recognized. The country has a strong tradition in design and architecture, with influential movements like De Stijl and architects like Rem Koolhaas. English is widely spoken, making the Netherlands accessible to international visitors and businesses.",
  "fieldBoundaries": [
    {"docId": 20, "start": 0, "end": 11, "fieldType": "title", "sectionId": null},
    {"docId": 20, "start": 12, "end": 3500, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Netherlands"

# Norway (21)
cat > "$DIR/21.json" << 'EOFCOUNTRY'
{
  "id": 21,
  "slug": "norway",
  "title": "Norway",
  "excerpt": "A Nordic country known for its fjords, Northern Lights, and sovereign wealth fund",
  "href": "/countries/norway",
  "type": "page",
  "category": "nordic",
  "text": "Norway, officially the Kingdom of Norway, is a Nordic country in Northern Europe. The mainland territory comprises the western and northernmost portion of the Scandinavian Peninsula. The remote Arctic island of Jan Mayen and the archipelago of Svalbard also form part of Norway. Bouvet Island, located in the Subantarctic, is a dependent territory of Norway. Norway also lays claim to the Antarctic territories of Peter I Island and Queen Maud Land.\n\nNorway shares a long eastern border with Sweden, while its northeastern border is with Finland and Russia. The country has an extensive coastline, facing the North Atlantic Ocean and the Barents Sea. The coast is famous for its fjords, deep inlets carved by glaciers over millions of years. Norway has a total area of 385,207 square kilometers and a population of approximately 5.4 million. Oslo is the capital and largest city.\n\nNorway has a long history stretching back to prehistoric times. The Viking Age (793-1066) saw Norwegian Vikings establish settlements across Europe and the North Atlantic, reaching as far as North America. Norway was united into one kingdom around 872 under Harald Fairhair. The country entered unions with Denmark (1380-1814) and Sweden (1814-1905) before gaining full independence in 1905.\n\nNorway is a highly developed country with one of the highest standards of living in the world. It has the world's largest sovereign wealth fund, the Government Pension Fund Global, funded largely by petroleum revenues. Despite its oil wealth, Norway has invested heavily in renewable energy, particularly hydroelectric power, which provides about 95 percent of the country's electricity.\n\nThe Norwegian fjords are a UNESCO World Heritage Site and include the Geirangerfjord and Nærøyfjord. These dramatic landscapes attract millions of tourists annually. Northern Norway is one of the best places to see the Northern Lights (Aurora Borealis). The midnight sun phenomenon, where the sun doesn't set for weeks during summer, is experienced in areas north of the Arctic Circle.\n\nNorwegian culture has been influenced by its Viking heritage and its harsh northern climate. Traditional music includes distinctive folk songs and the Hardanger fiddle (hardingfele). Norwegian literature has produced notable authors including Henrik Ibsen, considered the father of modern drama, and Knut Hamsun, winner of the Nobel Prize in Literature. The country has a strong tradition in winter sports, particularly cross-country skiing and ski jumping.\n\nThe Norwegian economy is highly developed and mixed, with a combination of free market activity and large government ownership in key sectors. Key industries include petroleum and natural gas, shipping, seafood, and metals. Norway has one of the highest GDP per capita in the world. The country maintains a generous welfare state, with universal healthcare and free education.\n\nNorway is not a member of the European Union, though it is part of the European Economic Area (EEA) and the Schengen Area. It is a founding member of NATO and actively participates in international peacekeeping operations. Norwegian cuisine features seafood, particularly salmon, cod, and herring, as well as dairy products, berries, and game meat. Traditional dishes include lutefisk, rakfisk, and brunost (brown cheese).",
  "fieldBoundaries": [
    {"docId": 21, "start": 0, "end": 6, "fieldType": "title", "sectionId": null},
    {"docId": 21, "start": 7, "end": 3400, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Norway"

# Poland (22)
cat > "$DIR/22.json" << 'EOFCOUNTRY'
{
  "id": 22,
  "slug": "poland",
  "title": "Poland",
  "excerpt": "Central Europe's largest nation with a complex history and vibrant culture",
  "href": "/countries/poland",
  "type": "page",
  "category": "central",
  "text": "Poland, officially the Republic of Poland, is a country in Central Europe. It is divided into 16 administrative provinces called voivodeships, covering an area of 312,696 square kilometers. Poland has a population of over 38 million and is the fifth-most populous member state of the European Union. Warsaw is the capital and largest city. Other major cities include Kraków, Wrocław, Łódź, Poznań, Gdańsk, and Szczecin.\n\nPoland is bordered by Germany to the west, the Czech Republic and Slovakia to the south, Ukraine and Belarus to the east, and Lithuania and the Russian exclave of Kaliningrad to the northeast. Poland also shares a maritime boundary with Denmark and Sweden. The country's geography includes the Baltic Sea coast in the north, the Carpathian Mountains in the south, and the Polish Plain in between.\n\nPoland has a rich and complex history. The Polish state was founded over 1,000 years ago, and Poland was one of the largest and most powerful countries in Europe during the Polish-Lithuanian Commonwealth (1569-1795). The country was partitioned among Russia, Prussia, and Austria in the late 18th century and did not regain independence until 1918. Poland suffered enormously during World War II, including the Holocaust, in which six million Polish citizens were killed, including three million Jews.\n\nAfter World War II, Poland became a communist state under Soviet influence until 1989. The Solidarity movement, founded in 1980, was the first independent trade union in a Soviet-bloc country and played a key role in the peaceful transition to democracy. Poland joined NATO in 1999 and the European Union in 2004. The country has experienced significant economic growth since the end of communism.\n\nPolish culture has made significant contributions to world heritage. Famous Poles include Nicolaus Copernicus (astronomer), Frédéric Chopin (composer), Marie Curie (physicist and chemist, the only person to win Nobel Prizes in two different sciences), and Pope John Paul II. Kraków, the former royal capital, is home to one of Europe's oldest universities (Jagiellonian University, founded 1364) and has been designated a UNESCO City of Literature.\n\nThe Polish economy is the sixth-largest in the European Union and has been one of the fastest-growing economies in Europe. Key sectors include manufacturing, services, agriculture, and mining. Poland is a major producer of coal, copper, and silver. The country has developed a significant IT sector and is home to numerous video game companies, including CD Projekt, creator of The Witcher series.\n\nPolish cuisine is hearty and flavorful, featuring dishes such as pierogi (dumplings), bigos (hunter's stew), żurek (sour rye soup), and kielbasa (sausage). The country is known for its vodka production. Traditional Polish folk culture, including music, dance, and handicrafts, remains vibrant, particularly in rural areas.\n\nPoland has 17 UNESCO World Heritage Sites, including the historic centers of Kraków and Warsaw, the medieval town of Toruń, and the Białowieża Forest, one of the last and largest remaining parts of the primeval forest that once spread across the European Plain. The country's landscape includes over 9,000 lakes, making it one of the most lake-rich countries in the world.",
  "fieldBoundaries": [
    {"docId": 22, "start": 0, "end": 6, "fieldType": "title", "sectionId": null},
    {"docId": 22, "start": 7, "end": 3500, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Poland"

# Portugal (23)
cat > "$DIR/23.json" << 'EOFCOUNTRY'
{
  "id": 23,
  "slug": "portugal",
  "title": "Portugal",
  "excerpt": "A historic seafaring nation on the Iberian Peninsula with Atlantic coastline",
  "href": "/countries/portugal",
  "type": "page",
  "category": "mediterranean",
  "text": "Portugal, officially the Portuguese Republic, is a country located on the Iberian Peninsula in southwestern Europe. It is the westernmost sovereign state of mainland Europe. The country is bordered by Spain to the north and east and the Atlantic Ocean to the west and south. Portugal also includes the Atlantic archipelagos of the Azores and Madeira, both autonomous regions with their own regional governments. The official language is Portuguese.\n\nPortugal has a total area of 92,212 square kilometers and a population of approximately 10 million. Lisbon is the capital and largest city, located at the mouth of the Tagus River on the Atlantic coast. Other major cities include Porto, Braga, Coimbra, and Funchal (in Madeira). The country has a Mediterranean climate with hot, dry summers and mild, rainy winters.\n\nPortugal is one of the oldest nation-states in Europe, with its borders largely unchanged since the 12th century. The country was the pioneer of the Age of Discovery in the 15th and 16th centuries, establishing the first global maritime empire. Portuguese explorers such as Vasco da Gama, Pedro Álvares Cabral, and Ferdinand Magellan opened sea routes to India, Brazil, and the Pacific. Portugal's colonial empire spanned Africa, Asia, and South America and lasted until 1999.\n\nAfter a long period of dictatorship under António de Oliveira Salazar (1932-1968) and Marcelo Caetano (1968-1974), the Carnation Revolution of April 25, 1974, established a democracy. Portugal joined the European Economic Community (now the European Union) in 1986 and was a founding member of the Eurozone. The country has undergone significant modernization since joining the EU.\n\nPortuguese culture is known for fado music, a melancholic genre of traditional music that expresses longing and saudade (a deep emotional state of nostalgic longing). Fado originated in Lisbon in the 1820s and is now a UNESCO Intangible Cultural Heritage. Portuguese literature has produced notable authors including Luís de Camões, Fernando Pessoa, and José Saramago, winner of the Nobel Prize in Literature.\n\nThe Portuguese economy is based on services, industry, and agriculture. Key sectors include tourism, textiles, wine, cork, and renewable energy. Portugal is the world's largest producer of cork, accounting for about 50 percent of world production. The country has invested heavily in renewable energy and at times generates more than 100 percent of its electricity needs from renewable sources.\n\nPortuguese cuisine is characterized by fish and seafood dishes, particularly bacalhau (salt cod), for which there are said to be 365 recipes, one for each day of the year. Other traditional dishes include caldo verde (green soup), francesinha (a Porto specialty), and pastéis de nata (custard tarts). Portugal is also known for its port wine, produced in the Douro Valley, and vinho verde, a young wine from the Minho region.\n\nPortugal has 17 UNESCO World Heritage Sites, including the Tower of Belém in Lisbon, the Historic Centre of Porto, and the Cultural Landscape of Sintra. The Azores and Madeira are known for their natural beauty and unique ecosystems. Tourism is a major industry, with millions of visitors attracted to the beaches of the Algarve, the historic cities of Lisbon and Porto, and the Atlantic islands.",
  "fieldBoundaries": [
    {"docId": 23, "start": 0, "end": 8, "fieldType": "title", "sectionId": null},
    {"docId": 23, "start": 9, "end": 3400, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Portugal"

# Romania (24)
cat > "$DIR/24.json" << 'EOFCOUNTRY'
{
  "id": 24,
  "slug": "romania",
  "title": "Romania",
  "excerpt": "A Southeastern European country known for Transylvania and Carpathian Mountains",
  "href": "/countries/romania",
  "type": "page",
  "category": "eastern",
  "text": "Romania is a country in Southeastern Europe. It borders Bulgaria to the south, Ukraine to the north, Hungary to the west, Serbia to the southwest, Moldova to the east, and the Black Sea to the southeast. It has a predominantly temperate-continental climate. With a total area of 238,397 square kilometers, Romania is the twelfth-largest country in Europe and the seventh-most populous member state of the European Union with nearly 19 million inhabitants. Its capital and largest city is Bucharest.\n\nThe territory of Romania has been inhabited since the Paleolithic era. The Dacians, a Thracian people, established the kingdom of Dacia in the 1st century BC. The Roman Empire conquered parts of Dacia in 106 AD, and the area was a Roman province until 271 AD. The Roman influence is reflected in the Romanian language, which is a Romance language descended from Latin.\n\nThe medieval period saw the establishment of the principalities of Wallachia and Moldavia, which later came under Ottoman suzerainty. Transylvania, which had been part of the Kingdom of Hungary, became a semi-independent principality under Ottoman overlordship. The modern state of Romania was formed in 1859 through the union of Wallachia and Moldavia, and the country gained independence from the Ottoman Empire in 1877. Transylvania joined Romania in 1918 after World War I.\n\nRomania was a communist state from 1947 to 1989, ruled for much of that period by Nicolae Ceaușescu, whose regime was one of the most repressive in the Eastern Bloc. The Romanian Revolution of 1989 ended communist rule. Romania joined NATO in 2004 and the European Union in 2007. The country has experienced significant economic development since then.\n\nRomania is known for its natural beauty, including the Carpathian Mountains, the Danube Delta (a UNESCO World Heritage Site and one of Europe's largest wetlands), and the Black Sea coast. Transylvania is famous for medieval castles, including Bran Castle (often associated with the Dracula legend) and Peleș Castle. The painted churches of northern Moldavia and the wooden churches of Maramureș are UNESCO World Heritage Sites.\n\nRomanian culture has been shaped by its Latin heritage and Orthodox Christian tradition. Romanian literature has produced notable writers including Mihai Eminescu (the national poet), Ion Creangă, and Mircea Eliade. Romanian music includes traditional folk music, which has influenced contemporary composers like George Enescu. The country has a strong tradition in gymnastics, having produced legendary athletes such as Nadia Comăneci.\n\nThe Romanian economy is one of the fastest-growing in the European Union. Key sectors include automotive manufacturing, IT services, textiles, and agriculture. Romania has significant natural resources, including oil and natural gas. The IT sector has grown substantially, with cities like Bucharest and Cluj-Napoca becoming regional tech hubs.\n\nRomanian cuisine features dishes such as sarmale (stuffed cabbage rolls), mici (grilled ground meat rolls), mămăligă (polenta), and ciorbă (sour soup). The country produces wines, particularly in regions like Dealu Mare and Cotnari. Traditional Romanian folk culture, including music, dance, and crafts, remains vibrant, especially in rural areas like Maramureș.",
  "fieldBoundaries": [
    {"docId": 24, "start": 0, "end": 7, "fieldType": "title", "sectionId": null},
    {"docId": 24, "start": 8, "end": 3400, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Romania"

# Slovakia (25)
cat > "$DIR/25.json" << 'EOFCOUNTRY'
{
  "id": 25,
  "slug": "slovakia",
  "title": "Slovakia",
  "excerpt": "A Central European country known for its castles and Tatra Mountains",
  "href": "/countries/slovakia",
  "type": "page",
  "category": "central",
  "text": "Slovakia, officially the Slovak Republic, is a landlocked country in Central Europe. It is bordered by Poland to the north, Ukraine to the east, Hungary to the south, Austria to the southwest, and the Czech Republic to the northwest. Slovakia's territory spans about 49,000 square kilometers, and is mostly mountainous. The population is over 5.4 million, and the capital and largest city is Bratislava, located on the border with Austria and Hungary.\n\nSlovakia has been part of various kingdoms and empires throughout its history. The territory was part of the Hungarian Kingdom for nearly 1,000 years (1000-1918). After World War I, Slovakia became part of Czechoslovakia, a new nation formed from the dissolution of Austria-Hungary. During World War II, Slovakia briefly existed as a German client state. After the war, Czechoslovakia was re-established and later became a communist state.\n\nThe Velvet Revolution of 1989 ended communist rule in Czechoslovakia. On January 1, 1993, Czechoslovakia peacefully dissolved into two independent states: the Czech Republic and Slovakia, in an event known as the Velvet Divorce. Slovakia joined NATO in 2004, the European Union in 2004, and the Eurozone in 2009.\n\nSlovakia is known for its natural beauty, particularly the High Tatras mountains, the smallest alpine mountain range in the world. The country has numerous caves, including the Domica cave system, part of the Aggtelek Karst, a UNESCO World Heritage Site. Slovakia has the highest number of castles and châteaux per capita in the world, including Spiš Castle, one of the largest castle sites in Central Europe.\n\nSlovak culture blends Western and Eastern European influences. Traditional Slovak folk culture includes distinctive music, dance, and costumes that vary by region. The fujara, a traditional Slovak shepherd's flute, is inscribed on UNESCO's list of Intangible Cultural Heritage. Slovak literature and arts have been influenced by both Czech and Hungarian cultural traditions.\n\nThe Slovak language is a Slavic language closely related to Czech. Slovaks and Czechs can generally understand each other due to the similarity of their languages, a legacy of their shared history in Czechoslovakia. Hungarian is also spoken in southern Slovakia, where there is a significant Hungarian minority.\n\nThe Slovak economy has grown significantly since the country's independence. Key industries include automotive manufacturing (Slovakia produces more cars per capita than any other country), electronics, and machinery. Major car manufacturers including Volkswagen, Kia, and Peugeot have factories in Slovakia. The country has attracted significant foreign investment due to its skilled workforce and strategic location.\n\nSlovak cuisine features hearty dishes influenced by Hungarian, Czech, and Austrian cuisines. Traditional dishes include bryndzové halušky (potato dumplings with sheep cheese), kapustnica (cabbage soup), and various types of sausages and smoked meats. Slovakia produces distinctive wines, particularly in the Small Carpathian wine region, and spirits including slivovica (plum brandy).\n\nSlovakia has seven UNESCO World Heritage Sites, including the historic town of Banská Štiavnica, the Vlkolínec folk architecture reserve, and the primeval beech forests of the Carpathians. The country's thermal springs and spas have been popular since Roman times.",
  "fieldBoundaries": [
    {"docId": 25, "start": 0, "end": 8, "fieldType": "title", "sectionId": null},
    {"docId": 25, "start": 9, "end": 3300, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Slovakia"

# Slovenia (26)
cat > "$DIR/26.json" << 'EOFCOUNTRY'
{
  "id": 26,
  "slug": "slovenia",
  "title": "Slovenia",
  "excerpt": "A small Alpine country bridging Central Europe and the Mediterranean",
  "href": "/countries/slovenia",
  "type": "page",
  "category": "central",
  "text": "Slovenia, officially the Republic of Slovenia, is a country in Central Europe. It is bordered by Italy to the west, Austria to the north, Hungary to the northeast, Croatia to the south and southeast, and the Adriatic Sea to the southwest. Slovenia covers 20,273 square kilometers and has a population of 2.1 million. Ljubljana is the capital and largest city. The country is noted for its mountains, ski resorts, lakes, and its short (47 km) but picturesque coastline.\n\nThe Slovene lands were part of the Austro-Hungarian Empire until its dissolution at the end of World War I. In 1918, Slovenes joined with other South Slavic peoples in forming the Kingdom of Serbs, Croats and Slovenes, later renamed Yugoslavia. Following World War II, Slovenia became a founding member of the Socialist Federal Republic of Yugoslavia. Slovenia declared independence from Yugoslavia on June 25, 1991, following a brief ten-day war.\n\nSlovenia joined NATO and the European Union in 2004 and the Eurozone in 2007. It was the first former communist country to hold the presidency of the Council of the European Union (in 2008) and to adopt the euro as its currency. The country has made a successful transition to a democratic political system and market economy.\n\nSlovenia's landscape is remarkably diverse for its small size. The Julian Alps in the northwest include Mount Triglav, the highest peak at 2,864 meters, which appears on the national flag. Lake Bled, with its island church and clifftop castle, is one of the country's most iconic sights. The Postojna Cave is one of the largest karst caves in the world and home to the olm, a rare cave-dwelling salamander.\n\nSlovenian culture reflects both its Central European and Mediterranean influences. The Slovenian language is a South Slavic language with distinctive dual grammatical number. Traditional Slovenian folk culture includes distinctive music, dance, and crafts. The country has produced notable artists, scientists, and athletes, including architect Jože Plečnik, whose work shaped modern Ljubljana.\n\nThe Slovenian economy is small but highly developed, with a high GDP per capita compared to other Central and Eastern European countries. Key industries include manufacturing (particularly motor vehicles, electrical appliances, and pharmaceuticals), services, and tourism. Slovenia has significant forests covering about 60 percent of its territory, making it one of the most forested countries in Europe.\n\nSlovenian cuisine varies by region but generally features influences from Italian, Austrian, and Hungarian cooking. Traditional dishes include potica (nut roll), žlikrofi (stuffed dumplings from Idrija), and štruklji (rolled dumplings). The country produces quality wines, particularly in the Primorska and Podravje regions. Slovenia is known for its local honey production and has a strong beekeeping tradition.\n\nSlovenia has several UNESCO World Heritage Sites, including the prehistoric pile dwellings around the Alps and the mercury mining site at Idrija. The country has invested heavily in sustainable tourism and environmental protection. Ljubljana was named European Green Capital in 2016 for its urban development and environmental policies. The country offers diverse outdoor activities including skiing, hiking, cycling, and water sports.",
  "fieldBoundaries": [
    {"docId": 26, "start": 0, "end": 8, "fieldType": "title", "sectionId": null},
    {"docId": 26, "start": 9, "end": 3200, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Slovenia"

# Spain (27)
cat > "$DIR/27.json" << 'EOFCOUNTRY'
{
  "id": 27,
  "slug": "spain",
  "title": "Spain",
  "excerpt": "A diverse Mediterranean country known for its culture, cuisine, and history",
  "href": "/countries/spain",
  "type": "page",
  "category": "mediterranean",
  "text": "Spain, officially the Kingdom of Spain, is a country in Southwestern Europe with some pockets of territory across the Strait of Gibraltar and the Atlantic Ocean. Its continental European territory is situated on the Iberian Peninsula. Spain is bordered by Portugal to the west, and Gibraltar and Morocco to the south. In the northeast, it is bordered by France, Andorra, and the Bay of Biscay. With an area of 505,990 square kilometers, Spain is the second-largest country in Western Europe and the fourth-most populous, with a population of approximately 47 million.\n\nSpain's capital and largest city is Madrid. Other major urban areas include Barcelona, Valencia, Seville, Zaragoza, Málaga, Murcia, and Bilbao. Spain also includes the Balearic Islands in the Mediterranean, the Canary Islands in the Atlantic Ocean off the African coast, and two autonomous cities in North Africa, Ceuta and Melilla.\n\nThe Spanish territory was home to various ancient civilizations, including the Iberians, Celts, Phoenicians, Greeks, Carthaginians, and Romans. After the fall of the Western Roman Empire, the Visigoths ruled the peninsula. In 711, the Umayyad Caliphate invaded and established Al-Andalus, a period of Islamic rule that lasted for nearly 800 years in parts of the peninsula. The Reconquista, the Christian reconquest of the Iberian Peninsula, was completed in 1492 with the fall of Granada.\n\nSpain became a global empire in the 16th and 17th centuries, colonizing large parts of the Americas, Philippines, and other territories. The Spanish Golden Age saw remarkable achievements in literature (Cervantes, Lope de Vega) and art (El Greco, Velázquez). Spain's colonial empire declined over the 19th century, culminating in the loss of its remaining overseas territories in 1898. The 20th century was marked by the Spanish Civil War (1936-1939) and the subsequent dictatorship of Francisco Franco (1939-1975).\n\nAfter Franco's death, Spain transitioned to democracy under King Juan Carlos I. The 1978 constitution established Spain as a constitutional monarchy with a parliamentary system. Spain joined the European Economic Community (now the EU) in 1986 and adopted the euro in 1999. The country has become one of the most decentralized countries in Europe, with 17 autonomous communities and two autonomous cities.\n\nSpanish culture is rich and diverse, with regional variations reflecting the country's historical kingdoms. Flamenco music and dance originated in Andalusia and is now a UNESCO Intangible Cultural Heritage. Spain has produced world-renowned artists including Pablo Picasso, Salvador Dalí, and Joan Miró. Spanish architecture includes the works of Antoni Gaudí, whose Sagrada Família basilica in Barcelona is a UNESCO World Heritage Site.\n\nThe Spanish economy is the fourth-largest in the Eurozone and the fourteenth-largest in the world. Tourism is a major industry, with Spain being the second-most visited country in the world. Other important sectors include automotive manufacturing, agriculture (particularly wine, olive oil, and citrus fruits), and services. Spain is one of the world's leading wine producers.\n\nSpanish cuisine is varied and reflects regional traditions. Famous dishes include paella (from Valencia), tapas, jamón ibérico, gazpacho, and tortilla española. Spain has a strong food culture, with numerous markets, food festivals, and Michelin-starred restaurants. The Spanish tradition of late dining and socializing in bars and restaurants is an important part of the culture.",
  "fieldBoundaries": [
    {"docId": 27, "start": 0, "end": 5, "fieldType": "title", "sectionId": null},
    {"docId": 27, "start": 6, "end": 3600, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Spain"

# Sweden (28)
cat > "$DIR/28.json" << 'EOFCOUNTRY'
{
  "id": 28,
  "slug": "sweden",
  "title": "Sweden",
  "excerpt": "A Nordic country known for design, innovation, and extensive forests",
  "href": "/countries/sweden",
  "type": "page",
  "category": "nordic",
  "text": "Sweden, officially the Kingdom of Sweden, is a Nordic country on the Scandinavian Peninsula in Northern Europe. It borders Norway to the west and north, Finland to the northeast, and is connected to Denmark in the southwest by a bridge-tunnel across the Öresund Strait. At 450,295 square kilometers, Sweden is the largest country in Northern Europe, the third-largest country in the European Union by area, and the fifth largest in Europe. Sweden has a total population of approximately 10.4 million. Stockholm is the capital and largest city.\n\nSweden emerged as a unified country during the Middle Ages. In the 17th century, it expanded its territories to form the Swedish Empire, which became one of the great powers of Europe until the early 18th century. Swedish territories outside the Scandinavian Peninsula were gradually lost during the 18th and 19th centuries, ending with the annexation of present-day Finland by Russia in 1809. The last war in which Sweden was directly involved was in 1814, when Sweden by military means forced Norway into a personal union.\n\nSince then, Sweden has been at peace, maintaining an official policy of neutrality in foreign affairs. Sweden was one of the first countries in the world to establish a welfare state in the 20th century. Sweden joined the European Union in 1995 but maintained the Swedish krona as its currency. In 2024, Sweden became a member of NATO, ending more than 200 years of neutrality following Russia's invasion of Ukraine.\n\nSweden is a highly developed country with an advanced economy and high standard of living. It ranks highly in quality of life, health, education, equality, and human development. Sweden is known for its comprehensive social security system, universal healthcare, and free education through university level. The country has one of the lowest levels of income inequality in the world.\n\nSwedish culture has had a significant global influence, particularly in design, music, and innovation. IKEA, the world's largest furniture retailer, is Swedish, as are fashion brands H&M and Acne Studios. Swedish design is known for its clean, functional aesthetic. Sweden has a thriving music industry, having produced internationally successful artists including ABBA, Roxette, Robyn, and Avicii. The country is also known for its crime fiction, with authors like Stieg Larsson and Henning Mankell.\n\nSweden has a highly innovative economy with significant contributions from technology, automotive, and pharmaceutical sectors. Major Swedish companies include Volvo, Ericsson, Electrolux, and Spotify. The country has one of the highest rates of patent applications per capita in the world. Sweden is also a leader in sustainable development and renewable energy.\n\nSwedish cuisine has evolved significantly in recent years, with a growing focus on locally sourced and seasonal ingredients. Traditional dishes include meatballs (köttbullar), herring, and crispbread. The Swedish tradition of fika (coffee break with pastries) is an important part of the culture. Sweden celebrates several unique traditions, including Midsummer (the most celebrated Swedish holiday after Christmas), Lucia Day, and the Nobel Prize ceremonies.\n\nSweden's landscape is characterized by extensive forests, numerous lakes, and a long coastline. About 69 percent of Sweden is covered by forests. The country has 30 national parks and numerous nature reserves. Northern Sweden, part of Lapland, is home to the indigenous Sami people and offers opportunities to see the Northern Lights. The midnight sun phenomenon is experienced in summer north of the Arctic Circle.",
  "fieldBoundaries": [
    {"docId": 28, "start": 0, "end": 6, "fieldType": "title", "sectionId": null},
    {"docId": 28, "start": 7, "end": 3600, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Sweden"

# Switzerland (29)
cat > "$DIR/29.json" << 'EOFCOUNTRY'
{
  "id": 29,
  "slug": "switzerland",
  "title": "Switzerland",
  "excerpt": "A neutral Alpine nation known for banking, watches, and direct democracy",
  "href": "/countries/switzerland",
  "type": "page",
  "category": "central",
  "text": "Switzerland, officially the Swiss Confederation, is a landlocked country at the confluence of Western, Central, and Southern Europe. It is bordered by Italy to the south, France to the west, Germany to the north, and Austria and Liechtenstein to the east. Switzerland is geographically divided among the Swiss Plateau, the Alps, and the Jura, spanning a total area of 41,285 square kilometers. Although the Alps occupy the greater part of the territory, the Swiss population of approximately 8.7 million is concentrated mostly on the plateau, where the largest cities and economic centers are located.\n\nSwitzerland has four national languages: German (63.5%), French (22.5%), Italian (8.1%), and Romansh (0.5%). English is widely spoken as a second language. The country is divided into 26 cantons, each with its own constitution and government. Bern is the de facto capital and seat of federal authorities, while Zurich is the largest city and the country's main economic center. Geneva is a major center for diplomacy and hosts numerous international organizations.\n\nThe Old Swiss Confederacy was founded in 1291 as a defensive alliance among cantons. The Swiss Federal State established in 1848 is one of the oldest republics in the world. Switzerland has maintained a policy of armed neutrality since the early modern period and has not been in a state of war internationally since 1815. It is not a member of the European Union and joined the United Nations only in 2002. However, it participates in the Schengen Area and has numerous bilateral agreements with the EU.\n\nSwitzerland is known for its system of direct democracy, where citizens can propose changes to the constitution or request a referendum on any law passed by parliament. This system has led to frequent national votes on various issues. The country has a highly decentralized political system, with significant powers devolved to the cantons.\n\nThe Swiss economy is one of the most competitive and stable in the world. It is known for its banking sector, which prioritizes privacy and security, though banking secrecy has been reduced in recent years due to international pressure. Other key industries include pharmaceuticals (home to Novartis and Roche), watchmaking (Rolex, Patek Philippe, Omega), machinery, chemicals, and food products (Nestlé). Switzerland has the highest wealth per adult in the world and consistently ranks among the top countries in quality of life.\n\nSwiss culture reflects its linguistic diversity and its position at the crossroads of European traditions. Switzerland has made significant contributions to literature, art, architecture, music, and science. Notable Swiss include psychologist Carl Jung, architect Le Corbusier, artist Paul Klee, and physicist Albert Einstein (who developed his theory of relativity while working in Bern).\n\nSwiss cuisine varies by region but is generally characterized by cheese and chocolate. Famous dishes include fondue, raclette, and rösti. Swiss chocolate, including brands like Lindt, Toblerone, and Sprüngli, is world-renowned. The country also produces notable wines, particularly in the French-speaking Valais and Vaud regions.\n\nThe Swiss Alps are a major tourist destination, offering skiing, hiking, and mountaineering. The Matterhorn, a peak on the border with Italy, is one of the most iconic mountains in the world. Switzerland is home to numerous lakes, including Lake Geneva, Lake Zurich, and Lake Lucerne. The country has an excellent public transportation system, including the famous scenic railway routes.",
  "fieldBoundaries": [
    {"docId": 29, "start": 0, "end": 11, "fieldType": "title", "sectionId": null},
    {"docId": 29, "start": 12, "end": 3500, "fieldType": "content", "sectionId": "overview"}
  ]
}
EOFCOUNTRY
echo "Generated Switzerland"

echo ""
echo "Generated 30 country files in $DIR"
echo ""
echo "To test the CLI:"
echo "  cargo run -- index --input examples/eu-countries --output /tmp/sorex-output"
