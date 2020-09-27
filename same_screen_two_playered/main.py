from flask import Flask, render_template, redirect, request, url_for, jsonify
from play import initialise_game, receive_move
import random

app = Flask(__name__)

@app.route('/passData', methods=['GET', 'POST'])
def passData():
    if request.method == "POST":
        req = request.get_json()
        # receive_move(req)
        player_pos = receive_move(req)
        # return '200 ok'
        return jsonify(player_pos)
    # else:
    #     player_positions = get_positions(req)
    #     return player_positions

@app.route('/', methods=["GET", "POST"])
def signup():

    if request.method == "POST":
        req = request.form
        name1 = req.get("name1")
        name2 = req.get("name2")

        curr_game_name = 'game' + name1 + name2 + str(random.randrange(100))
        initialise_game(name1, name2, curr_game_name)

        return redirect(url_for('.play', name1=name1, name2=name2, curr_game_name=curr_game_name))

    return render_template("signup.html")

@app.route("/about")
def about():
    return render_template("about.html")

@app.route("/play")
def play():
    name1 = request.args['name1']
    name2 = request.args['name2']
    curr_game_name = request.args['curr_game_name']
    return render_template("play.html", messages={'name1':name1, 'name2':name2, 'curr_game_name':curr_game_name})

@app.route("/tic_tac_toe")
def tic_tac_toe():
    return render_template("tic_tac_toe/tic_tac_toe.html")

@app.route("/reading_club")
def reading_club():
    return render_template("reading_club/reading_club.html")

# Remove this section of code when uploading onto pythonanywhere.com
if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
