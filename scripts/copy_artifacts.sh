mkdir -p ./artifacts

mkdir -p ./artifacts/motherspace
cp ./target/ink/motherspace/motherspace.contract ./artifacts/motherspace
cp ./target/ink/motherspace/motherspace.json ./artifacts/motherspace
cp ./target/ink/motherspace/motherspace.wasm ./artifacts/motherspace

mkdir -p ./artifacts/space
cp ./target/ink/space/space.contract ./artifacts/space
cp ./target/ink/space/space.json ./artifacts/space
cp ./target/ink/space/space.wasm ./artifacts/space

mkdir -p ./artifacts/plugins

mkdir -p ./artifacts/plugins/posts

cp ./target/ink/posts/posts.contract ./artifacts/plugins/posts
cp ./target/ink/posts/posts.json ./artifacts/plugins/posts
cp ./target/ink/posts/posts.wasm ./artifacts/plugins/posts

mkdir -p ./artifacts/plugins/posts_launcher

cp ./target/ink/posts_launcher/posts_launcher.contract ./artifacts/plugins/posts_launcher
cp ./target/ink/posts_launcher/posts_launcher.json ./artifacts/plugins/posts_launcher
cp ./target/ink/posts_launcher/posts_launcher.wasm ./artifacts/plugins/posts_launcher


mkdir -p ./artifacts/plugins/flipper

cp ./target/ink/flipper/flipper.contract ./artifacts/plugins/flipper
cp ./target/ink/flipper/flipper.json ./artifacts/plugins/flipper
cp ./target/ink/flipper/flipper.wasm ./artifacts/plugins/flipper

mkdir -p ./artifacts/plugins/flipper_launcher

cp ./target/ink/flipper_launcher/flipper_launcher.contract ./artifacts/plugins/flipper_launcher
cp ./target/ink/flipper_launcher/flipper_launcher.json ./artifacts/plugins/flipper_launcher
cp ./target/ink/flipper_launcher/flipper_launcher.wasm ./artifacts/plugins/flipper_launcher


mkdir -p ./artifacts/plugins/polls

cp ./target/ink/polls/polls.contract ./artifacts/plugins/polls
cp ./target/ink/polls/polls.json ./artifacts/plugins/polls
cp ./target/ink/polls/polls.wasm ./artifacts/plugins/polls

mkdir -p ./artifacts/plugins/polls_launcher

cp ./target/ink/polls_launcher/polls_launcher.contract ./artifacts/plugins/polls_launcher
cp ./target/ink/polls_launcher/polls_launcher.json ./artifacts/plugins/polls_launcher
cp ./target/ink/polls_launcher/polls_launcher.wasm ./artifacts/plugins/polls_launcher

echo DONE!