from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

app = FastAPI()

class User(BaseModel):
    id: int
    name: str
    email: str

@app.get("/users/{user_id}")
def get_user(user_id: int):
    # TODO: Add database query
    return {"id": user_id, "name": "Test User"}

@app.post("/users")
def create_user(user: User):
    # TODO: Add validation and persistence
    return user
